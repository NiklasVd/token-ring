use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex}, collections::HashMap, net::{SocketAddr, SocketAddrV4, Ipv4Addr}};
use crossbeam_channel::{Sender, Receiver, unbounded};
use ed25519_dalek::Keypair;
use log::{warn, info};
use tokio::net::UdpSocket;
use crate::{id::WorkStationId, comm::{QueuedPacket, WorkStationSender, WorkStationReceiver, send_loop, recv_loop}, signature::{generate_keypair, Signed}, err::{TResult, GlobalError, TokenRingError}, packet::{Packet, PacketType, PacketHeader, JoinAnswerResult}, token::{Token, TokenHeader}, pass::{TokenPasser, StationStatus}};

pub type AMx<T> = Arc<Mutex<T>>;

pub fn create_amx<T>(val: T) -> AMx<T> {
    Arc::new(Mutex::new(val))
}

pub struct Config {
    pub id: WorkStationId,
    pub keypair: Keypair,
    pub accept_conns: bool
}

pub struct GlobalConfig {
    password: String,
    accept_connections: bool,
    max_connections: u16,
    max_passover_time: f32
}

impl GlobalConfig {
    pub fn new(password: String, accept_connections: bool, max_connections: u16,
        max_passover_time: f32) -> GlobalConfig {
        GlobalConfig {
            password, accept_connections, max_connections, max_passover_time
        }
    }
}

impl Config {
    pub fn new(id: WorkStationId) -> Config {
        let keypair = generate_keypair();
        Config {
            id, keypair, accept_conns: true
        }
    }
}

pub trait WorkStation {
    fn config(&self) -> Config;
    fn running(&self) -> bool;
}

pub struct ActiveStation {
    config: Config,
    global_config: GlobalConfig,
    sock: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
    connected_stations: HashMap<WorkStationId, SocketAddr>,
    token_passer: TokenPasser,

    send_queue: Sender<QueuedPacket>,
    recv_queue: Receiver<QueuedPacket>
}

impl ActiveStation {
    pub async fn host(id: WorkStationId, global_config: GlobalConfig, port: u16) -> TResult<ActiveStation> {
        // Bind socket to local addr and port and wrap into arc for passing to bg threads
        let sock = UdpSocket::bind(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED, port)).await?;
        let sock_arced = Arc::new(sock);
        let running = Arc::new(AtomicBool::new(true));

        // Sender handles all outgoing packets (serializing, transport) in a
        // background thread
        let send_queue = unbounded();
        let sender = WorkStationSender::new(running.clone(),
            sock_arced.clone(), send_queue.1);
        send_loop(sender)?;
        
        // Recv handles all incoming packets, deserializing, buffering
        // and event generation in a backtround thread
        let recv_queue = unbounded();
        let recv = WorkStationReceiver::new(
            running.clone(), sock_arced.clone(), recv_queue.0);
        recv_loop(recv)?;
        
        // The token passer stores current token rotating in the ring and
        // stores which stations already owned the token and in which
        // order and time it should be passed on.
        let token_passer = TokenPasser::new(global_config.max_passover_time);
        Ok(ActiveStation {
            config: Config::new(id), global_config: global_config,
            sock: sock_arced, running,
            connected_stations: HashMap::new(), token_passer,
            send_queue: send_queue.0, recv_queue: recv_queue.1
        })
    }

    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }

    async fn send_packet(&mut self, dest_addr: SocketAddr, dest_id: WorkStationId,
        packet: PacketType) -> TResult {
        let packet = Packet::new(
            // Move packet header signature into background send thread?
            // Hash generation is fast on eddsa algorithm but send loop exists for a reason 
            Signed::new(&self.config.keypair, 
                PacketHeader::new(self.config.id.clone(), dest_id))?, 
            packet);
        Ok(self.send_queue.send(QueuedPacket(packet, dest_addr))?)
    }

    // async fn recv_packet(&mut self) -> TResult<PacketType> {
    // }

    pub async fn recv_all(&mut self) -> TResult {
        while let Ok(packet) = self.recv_queue.try_recv() {
            let source_id = &packet.0.header.val.source;
            // Check signature and destination ID
            if let Err(e) = self.verify_recv_packet(&packet.0) {
                warn!("{:?}{:?} sent invalid packet: {e}. Data will be discarded.",
                    source_id, packet.1);
                return Err(e)
            } else {
                match packet.0.content {
                    PacketType::JoinRequest(pw) => 
                        self.recv_join_request(packet.1, source_id.clone(), pw).await?,
                    PacketType::JoinReply(_) => {
                        warn!("Received join reply by {:?}{:?} as active station. Discarding.", source_id, packet.1)
                    },
                    PacketType::TokenPass(token) => self.recv_token_pass(packet.1, source_id, token).await?,
                    PacketType::Leave() => self.recv_leave(packet. 1, source_id).await?,
                };
            }
        }
        Ok(())
    }

    async fn recv_join_request(&mut self, join_addr: SocketAddr, join_id: WorkStationId,
        pw: String) -> TResult {
        if let Some(addr) = self.get_station_addr(&join_id) {
            if addr == join_addr {
                warn!("{:?}{:?} attempted to join ring twice. Blocking attempt.", join_id, join_id);
                self.send_packet(addr, join_id.clone(), 
                    PacketType::JoinReply(
                        JoinAnswerResult::Deny("Already joined".to_owned()))).await?;
                return Err(GlobalError::Internal(
                    TokenRingError::RejectedJoinAttempt(join_id, "Already Joined".to_owned())))
            } else {
                // Work station joined again but with new socket addr.
                warn!("{:?}{:?} attempted to join with new socket addr {:?}. Passing.", join_id, addr, join_addr)
            }
        }

        if let Err(e) = self.check_join_request(&join_id, pw) {
            // TOOD: Improve deny reason
            self.send_packet(join_addr, join_id, 
                PacketType::JoinReply(
                    JoinAnswerResult::Deny("Invalid config".to_owned()))).await?;
            return Err(e)
        }
        
        let join_reply = PacketType::JoinReply(JoinAnswerResult::Confirm());
        self.send_packet(join_addr, join_id.clone(), 
            join_reply).await?;
        self.add_station(join_id.clone(), join_addr);
        info!("Added new station to ring: {:?}{:?}.", join_id, join_addr);
        Ok(())
    }

    fn check_join_request(&self, join_id: &WorkStationId, pw: String) -> TResult {
        let err = if !self.global_config.accept_connections {
            TokenRingError::RejectedJoinAttempt(
                join_id.clone(), "New connections blocked".to_owned())
        } else if self.connected_stations.len() >=
            self.global_config.max_connections as usize {
            TokenRingError::RejectedJoinAttempt(
                join_id.clone(), format!("Max connections reached ({})", self.global_config.max_connections))
        } else if self.global_config.password != pw {
            TokenRingError::RejectedJoinAttempt(
                join_id.clone(), "Incorrect password".to_owned())
        } else {
            return Ok(())
        };
        Err(GlobalError::Internal(err))
    }

    fn add_station(&mut self, id: WorkStationId, addr: SocketAddr) {
        if let Some(prev_station) = self.connected_stations.insert(
            id.clone(), addr) {
            warn!("New station has same ID as {:?}{:?}. Replacing contact.", id, prev_station);
        } else {
            // If this ID didnt exist before, add to status list
            self.token_passer.station_status.insert(id, StationStatus(false));
        }
    }

    fn remove_station(&mut self, id: &WorkStationId) {
        if let Some(_) = self.connected_stations.remove(id) {
            self.token_passer.station_status.remove(id);
        } else {
            warn!("Did not find connected station with id {id}.")
        }
    }

    fn get_station_addr(&mut self, id: &WorkStationId) -> Option<SocketAddr> {
        self.connected_stations.get(id).copied()
    }

    async fn recv_token_pass(&mut self, addr: SocketAddr, id: &WorkStationId, token: Token) -> TResult {
        // Check if socket addr of token sender equals addr stored in id hashmap
        if let Some(station_addr) = self.get_station_addr(id) {
            if station_addr != addr {
                warn!("{:?}{:?} passed token but is registered under socket addr {:?}. Discarding token.", id, addr, station_addr);
                return Err(GlobalError::Internal(TokenRingError::InvalidToken(id.clone(), token)));
            }
        }
        self.token_passer.recv_token(token, id)
    }

    pub async fn poll_token_pass(&mut self) -> TResult {
        if self.token_passer.pass_ready() {
            self.pass_on_token().await
        } else {
            Err(GlobalError::Internal(TokenRingError::TokenPending))
        }
    }

    async fn pass_on_token(&mut self) -> TResult {
        if let Some(next_station) = self.token_passer.select_next_station() {
            let addr = self.get_station_addr(&next_station).unwrap();
            let curr_token = match self.token_passer.curr_token.as_ref() {
                Some(t) => {
                    info!("Passing token on to {:?}{:?}.", next_station, addr);
                    t.clone()
                },
                None => {
                    info!("Token passed over all stations. Generating new and passing to {:?}{:?}.", next_station, addr);
                    self.generate_token()?
                }
            };

            self.send_packet(addr, next_station.clone(), 
                PacketType::TokenPass(curr_token)).await
        } else {
            warn!("Cannot pass on token because ring is empty.");
            Err(GlobalError::Internal(TokenRingError::EmptyRing))
        }
    }

    async fn recv_leave(&mut self, addr: SocketAddr, id: &WorkStationId) -> TResult {
        if let Some(registered_addr) = self.get_station_addr(id) {
            if registered_addr == addr {
                info!("{:?}{:?} left the ring.", id, addr);
                self.remove_station(id);
                return Ok(())
            } else {
                warn!("{:?}{:?} intended to leave ring but registered socket addr differs: {:?}. Ignoring.", id, addr, registered_addr);
            }
        } else {
            warn!("{:?}{:?} intended to leave but is not a registered station in this ring.", id, addr)
        }
        Err(GlobalError::Internal(TokenRingError::StationNotRegistered(id.clone(), addr)))
    }

    fn generate_token(&self) -> TResult<Token> {
        Ok(Token::new(Signed::new(
                    &self.config.keypair, TokenHeader::new(
                        self.config.id.clone()))?))
    }

    fn verify_recv_packet(&self, packet: &Packet) -> TResult {
        if packet.header.verify() {
            if packet.header.val.destination != self.config.id {
                Err(GlobalError::Internal(
                    TokenRingError::InvalidWorkStationId(
                        packet.header.val.destination.clone(), self.config.id.clone())))
            } else {
                Ok(())
            }
        } else {
            Err(GlobalError::Internal(TokenRingError::InvalidSignature))
        }
    }
}

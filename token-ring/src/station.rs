use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex}, collections::HashMap, net::{SocketAddr, SocketAddrV4, Ipv4Addr}, time::Duration};
use crossbeam_channel::{Sender, Receiver, unbounded};
use ed25519_dalek::Keypair;
use tokio::net::UdpSocket;
use crate::{id::WorkStationId, comm::{QueuedPacket, WorkStationSender, WorkStationReceiver, send_loop, recv_loop}, signature::{generate_keypair, Signed}, err::{TResult, GlobalError, TokenRingError}, packet::{Packet, PacketType, PacketHeader, JoinAnswerResult}, token::{Token, TokenHeader, TokenFrame, TokenFrameType, TokenFrameId}, pass::{TokenPasser, StationStatus}};

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

    async fn send_packet(&mut self, dest_addr: SocketAddr,
        packet: PacketType) -> TResult {
        let packet = Packet::new(
            // Move packet header signature into background send thread?
            // Hash generation is fast on eddsa algorithm but send loop exists for a reason 
            Signed::new(&self.config.keypair, 
                PacketHeader::new(self.config.id.clone()))?, 
            packet);
        Ok(self.send_queue.send(QueuedPacket(packet, dest_addr))?)
    }

    // async fn recv_packet(&mut self) -> TResult<PacketType> {
    // }

    pub async fn recv_all(&mut self) -> TResult {
        while let Ok(packet) = self.recv_queue.try_recv() {
            let source_id = &packet.0.header.val.source;
            // Check signature and destination ID
            if let Err(e) = self.verify_recv_packet(&packet) {
                println!("{:?}{:?} sent invalid packet: {e}. Data will be discarded.",
                    source_id, packet.1);
                return Err(e)
            } else {
                match packet.0.content {
                    PacketType::JoinRequest(pw) => 
                        self.recv_join_request(packet.1, source_id.clone(), pw).await?,
                    PacketType::JoinReply(_) => {
                        println!("Received join reply by {:?}{:?} as active station. Discarding.", source_id, packet.1)
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
                println!("{:?}{:?} attempted to join ring twice. Blocking attempt.", join_id, join_id);
                self.send_packet(addr, 
                    PacketType::JoinReply(
                        JoinAnswerResult::Deny("Already joined".to_owned()))).await?;
                return Err(GlobalError::Internal(
                    TokenRingError::RejectedJoinAttempt(join_id, "Already Joined".to_owned())))
            } else {
                // Work station joined again but with new socket addr.
                println!("{:?}{:?} attempted to join with new socket addr {:?}. Passing.", join_id, addr, join_addr)
            }
        }

        if let Err(e) = self.check_join_request(&join_id, pw) {
            // TOOD: Improve deny reason
            self.send_packet(join_addr, 
                PacketType::JoinReply(
                    JoinAnswerResult::Deny("Invalid config".to_owned()))).await?;
            return Err(e)
        } else {
            let join_reply = PacketType::JoinReply(JoinAnswerResult::Confirm(self.config.id.clone()));
            self.send_packet(join_addr, 
                join_reply).await?;
            self.add_station(join_id.clone(), join_addr);

            println!("Added new station to ring: {:?}{:?}.", join_id, join_addr);
            Ok(())
        }
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
            println!("New station has same ID as {:?}{:?}. Replacing contact.", id, prev_station);
        } else {
            // If this ID didnt exist before, add to status list
            self.token_passer.station_status.insert(id, StationStatus(false));
        }
    }

    fn remove_station(&mut self, id: &WorkStationId) {
        if let Some(_) = self.connected_stations.remove(id) {
            self.token_passer.station_status.remove(id);
        } else {
            println!("Did not find connected station with id {id}.")
        }
    }

    fn get_station_addr(&self, id: &WorkStationId) -> Option<SocketAddr> {
        self.connected_stations.get(id).copied()
    }

    async fn recv_token_pass(&mut self, addr: SocketAddr, id: &WorkStationId, token: Token) -> TResult {
        // Check if socket addr of token sender equals addr stored in id hashmap
        if let Some(station_addr) = self.get_station_addr(id) {
            if station_addr != addr {
                println!("{:?}{:?} passed token but is registered under socket addr {:?}. Discarding token.", id, addr, station_addr);
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
        let next_station = if let Some(next_station) = self.token_passer.select_next_station() {
            next_station
        } else {
            println!("Cannot pass on token because ring is empty.");
            return Err(GlobalError::Internal(TokenRingError::EmptyRing))
        };
        let addr = self.get_station_addr(&next_station).unwrap();
        let curr_token = match self.token_passer.curr_token.as_ref() {
            Some(t) => {
                println!("Passing token on to {:?}{:?}.", next_station, addr);  
                t.clone()
            },
            None => {
                println!("Token passed over all stations. Generating new and passing to {:?}{:?}.", next_station, addr);
                self.generate_token()?
            }
        };

        self.token_passer.pass_token(next_station);
        self.send_packet(addr, 
            PacketType::TokenPass(curr_token)).await
    }

    async fn recv_leave(&mut self, addr: SocketAddr, id: &WorkStationId) -> TResult {
        if let Some(registered_addr) = self.get_station_addr(id) {
            if registered_addr == addr {
                println!("{:?}{:?} left the ring.", id, addr);
                self.remove_station(id);
                return Ok(())
            } else {
                println!("{:?}{:?} intended to leave ring but registered socket addr differs: {:?}. Ignoring.", id, addr, registered_addr);
            }
        } else {
            println!("{:?}{:?} intended to leave but is not a registered station in this ring.", id, addr)
        }
        Err(GlobalError::Internal(TokenRingError::StationNotRegistered(id.clone(), addr)))
    }

    fn generate_token(&self) -> TResult<Token> {
        Ok(Token::new(Signed::new(
                    &self.config.keypair, TokenHeader::new(
                        self.config.id.clone()))?))
    }

    fn verify_recv_packet(&self, packet: &QueuedPacket) -> TResult {
        if packet.0.header.verify() {
            match packet.0.content {
                PacketType::JoinRequest(_) => Ok(()),
                _ => {
                    if let None = self.get_station_addr(
                        &packet.0.header.val.source).as_ref() {
                        Err(GlobalError::Internal(TokenRingError::StationNotRegistered(
                            packet.0.header.val.source.clone(), packet.1)))
                    } else {
                        Ok(())
                    }
                }
            }
        } else {
            Err(GlobalError::Internal(TokenRingError::InvalidSignature))
        }
    }
}

pub enum ConnectionMode {
    Offline,
    Pending(SocketAddr),
    Connected(WorkStationId, SocketAddr)
}

pub struct PassiveStation {
    config: Config,
    sock: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
    conn_mode: ConnectionMode,
    cached_frames: Vec<TokenFrame>,
    curr_token: Option<Token>,

    send_queue: Sender<QueuedPacket>,
    recv_queue: Receiver<QueuedPacket>
}

impl PassiveStation {
    pub async fn new(id: WorkStationId, port: u16) -> TResult<PassiveStation> {
        let sock = UdpSocket::bind(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED, port)).await?;
        let sock_arced = Arc::new(sock);
        let running = Arc::new(AtomicBool::new(true));

        let send_queue = unbounded();
        let sender = WorkStationSender::new(running.clone(),
            sock_arced.clone(), send_queue.1);
        send_loop(sender)?;

        let recv_queue = unbounded();
        let recv = WorkStationReceiver::new(running.clone(),
            sock_arced.clone(), recv_queue.0);
        recv_loop(recv)?;

        Ok(PassiveStation {
            config: Config::new(id), sock: sock_arced.clone(), running,
            conn_mode: ConnectionMode::Offline, cached_frames: vec![],
            curr_token: None,
            send_queue: send_queue.0, recv_queue: recv_queue.1
        })
    }

    pub async fn connect(&mut self, addr: SocketAddr, pw: String) -> TResult {
        self.send_packet_to(addr, PacketType::JoinRequest(pw))?;
        self.conn_mode = ConnectionMode::Pending(addr);
        Ok(())
    }

    pub async fn shutdown(&mut self) -> TResult {
        self.send_packet(PacketType::Leave())?;
        // Sleep on main thread for 1 sec so that background thread can
        // send goodbye in time.
        tokio::time::sleep(Duration::from_secs(2)).await;
        self.running.store(false, Ordering::Relaxed);
        self.conn_mode = ConnectionMode::Offline;
        println!("Shutdown passive station {}.", self.config.id);
        Ok(())
    }

    pub fn append_frame(&mut self, frame: TokenFrameType) {
        println!("Appended token frame {:?} for next token.", frame);
        self.cached_frames.push(TokenFrame::new(TokenFrameId::new(
            self.config.id.clone()), frame));
    }

    pub fn get_token_mut(&mut self) -> Option<&mut Token> {
        self.curr_token.as_mut()
    }

    pub fn pass_on_token(&mut self) -> TResult {
        if let Some(curr_token) = self.curr_token.take() {
            self.send_packet(PacketType::TokenPass(curr_token))
        } else {
            Err(GlobalError::Internal(TokenRingError::TokenPending))
        }
    }

    pub async fn recv_next(&mut self) -> TResult {
        if let Ok(packet) = self.recv_queue.try_recv() {
            match &self.conn_mode {
                ConnectionMode::Connected(
                    target_id, target_addr) => {
                        // Already connected. Is received packet from this connection (active station)?
                        if &packet.1 == target_addr {
                            if &packet.0.header.val.source == target_id {
                                // Packet is legit; continue.
                                match packet.0.content {
                                    PacketType::TokenPass(token) => self.recv_token_pass(token),
                                    n @ _ => println!("Received invalid packet type: {:?}.", n)
                                }
                                Ok(())
                            } else {
                                Err(GlobalError::Internal(
                                    TokenRingError::InvalidWorkStationId(packet.0.header.val.source, target_id.clone())))
                            }
                        } else {
                            Err(GlobalError::Internal(TokenRingError::InvalidSocketAddress(packet.1)))
                        }
                    },
                    _ =>  {
                        match packet.0.content {
                            PacketType::JoinReply(result) => {
                                self.recv_join_reply(result).await
                            },
                            n @ _ => {
                                println!("Received invalid packet: {:?}. Local station is not connected yet.", n);
                                Err(GlobalError::Internal(TokenRingError::NotConnected))
                        }
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    async fn recv_join_reply(&mut self, result: JoinAnswerResult) -> TResult {
        let addr = match &self.conn_mode {
            ConnectionMode::Offline => {
                println!("Received join reply without asking. Discarding.");
                return Err(GlobalError::Internal(TokenRingError::NotConnected))
            },
            ConnectionMode::Connected(_, _) => {
                println!("Received join reply but station is already connected. Discarding.");
                return Err(GlobalError::Internal(TokenRingError::AlreadyConnected))
            },
            ConnectionMode::Pending(addr) => *addr
        };

        match result {
            JoinAnswerResult::Confirm(id) => {
                println!("Active station {id} accepted connection. Joining ring.");
                self.conn_mode = ConnectionMode::Connected(id, addr);
                Ok(())
            },
            JoinAnswerResult::Deny(reason) => {
                println!("Active workstation denied access: {reason}.");
                Err(GlobalError::Internal(TokenRingError::FailedJoinAttempt(reason)))
            },
        }
    }

    fn recv_token_pass(&mut self, mut token: Token) {
        if let Some(prev_token) = self.curr_token.as_ref() {
            println!("Already holding token: {:?}. Discarding old and accepting new one.", prev_token)
        }
        // Move all cached frames into new token.
        token.frames.append(&mut self.cached_frames.drain(..).collect::<Vec<_>>());
        self.curr_token = Some(token);
    }

    fn send_packet_to(&mut self, addr: SocketAddr, packet: PacketType) -> TResult {
        let packet = Packet::new(
            // Move packet header signature into background send thread?
            // Hash generation is fast on eddsa algorithm but send loop exists for a reason 
            Signed::new(&self.config.keypair, 
                PacketHeader::new(self.config.id.clone()))?, packet);
        Ok(self.send_queue.send(QueuedPacket(packet, addr))?)
    }

    fn send_packet(&mut self, packet: PacketType) -> TResult {
        match &self.conn_mode {
            ConnectionMode::Connected(_, addr) =>
                self.send_packet_to(*addr, packet),
            _ => Err(GlobalError::Internal(TokenRingError::NotConnected))
        }
    }
}

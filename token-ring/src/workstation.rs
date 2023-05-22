use std::{net::{SocketAddr, SocketAddrV4, Ipv4Addr}, sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex}, collections::HashMap};
use crossbeam_channel::unbounded;
use ed25519_dalek::{Keypair};
use log::error;
use tokio::net::UdpSocket;
use crate::{id::WorkStationId, err::{TResult, GlobalError, TokenRingError}, comm::{WorkStationSender, WorkStationReceiver, Sx, Rx, send_loop, recv_loop, QueuedPacket}, packet::{Packet, PacketHeader, PacketType}, signature::{Signed, generate_keypair}};

pub type AMx<T> = Arc<Mutex<T>>;

pub fn create_amx<T>(val: T) -> AMx<T> {
    Arc::new(Mutex::new(val))
}

pub struct Config {
    pub id: WorkStationId,
    pub keypair: Keypair,
    pub accept_conns: bool
}

impl Config {
    pub fn new(id: WorkStationId) -> Config {
        let keypair = generate_keypair();
        Config {
            id, keypair, accept_conns: true
        }
    }
}

pub enum ConnectionMode {
    Offline,
    Request(WorkStationId),
    RingMember {
        back: WorkStationId, // Work station that handled join request
        front: WorkStationId // Next work station in line, referred to by 'back' station 
    }
}

pub struct WorkStation {
    config: Config,
    stored_ids: HashMap<WorkStationId, SocketAddr>,
    conn_mode: ConnectionMode,

    sock: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
    send_queue: Sx<QueuedPacket>,
    recv_queue: Rx<QueuedPacket>
}

impl WorkStation {
    pub async fn setup(config: Config, port: u16) -> TResult<WorkStation> {
        let sock = UdpSocket::bind(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED, port)).await?;
        let sock_arced = Arc::new(sock);
        let running = Arc::new(AtomicBool::new(true));

        let send_queue = unbounded();
        let sender = WorkStationSender::new(running.clone(),
            sock_arced.clone(), send_queue.1);
        send_loop(sender)?;
        
        let recv_queue = unbounded();
        let recv = WorkStationReceiver::new(
            running.clone(), sock_arced.clone(), recv_queue.0);
        recv_loop(recv)?;

        Ok(WorkStation {
            config, stored_ids: HashMap::new(), conn_mode: ConnectionMode::Offline,
            sock: sock_arced, running, send_queue: send_queue.0, recv_queue: recv_queue.1
        })
    }

    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub async fn send_packet(&mut self, dest_addr: SocketAddr, dest_id: WorkStationId,
        packet: PacketType) -> TResult {
        let packet = Packet::new(
            Signed::new(&self.config.keypair, 
                PacketHeader::new(self.config.id.clone(), dest_id))?, 
            packet);
        Ok(self.send_queue.send(QueuedPacket(packet, dest_addr))?)
    }

    pub async fn join_ring(&mut self, dest_addr: SocketAddr, dest_id: WorkStationId) -> TResult {
        let packet = PacketType::JoinRequest(self.config.id.clone());
        self.send_packet(dest_addr, dest_id.clone(), packet).await?;
        self.conn_mode = ConnectionMode::Request(dest_id.clone());
        Ok(())
    }

    pub async fn leave_ring(&mut self) -> TResult {
        match &self.conn_mode {
            ConnectionMode::RingMember { back, .. } => {
                if let Some(back_addr) = self.lookup_id(back) {
                    self.send_packet(*back_addr, back.clone(),
                        PacketType::Leave(back.clone())).await
                } else {
                    error!("Failed to look up back station {back}.");
                    Ok(())
                }
            },
            _ => Err(GlobalError::Internal(TokenRingError::NotConnected))
        }
    }



    fn lookup_id(&self, id: &WorkStationId) -> Option<&SocketAddr> {
        self.stored_ids.get(id)
    }
}

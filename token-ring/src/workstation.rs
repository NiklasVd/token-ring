use std::{net::{SocketAddr, SocketAddrV4, Ipv4Addr}, sync::{Arc, atomic::AtomicBool, Mutex}};
use crossbeam_channel::unbounded;
use tokio::net::UdpSocket;
use crate::{id::WorkStationId, err::TResult, comm::{WorkStationSender, WorkStationReceiver, Sx, Rx, send_loop, recv_loop, QueuedPacket}, packet::Packet};

pub type AMx<T> = Arc<Mutex<T>>;

pub fn create_amx<T>(val: T) -> AMx<T> {
    Arc::new(Mutex::new(val))
}

pub struct WorkStation {
    id: WorkStationId,
    sock: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
    send_queue: Sx<QueuedPacket>,
    recv_queue: Rx<QueuedPacket>
}

impl WorkStation {
    pub async fn connect(id: WorkStationId, port: u16, target_addr: SocketAddr) -> TResult<WorkStation> {
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
            id, sock: sock_arced, running, send_queue: send_queue.0, recv_queue: recv_queue.1
        })
    }
}

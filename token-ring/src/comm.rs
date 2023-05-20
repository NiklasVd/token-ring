use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, net::SocketAddr};
use crossbeam_channel::{Sender, Receiver};
use tokio::net::UdpSocket;
use crate::{packet::Packet, err::TResult};

pub type Sx<T> = Sender<T>;
pub type Rx<T> = Receiver<T>;
pub type Channel<T> = (Sx<T>, Rx<T>);

pub struct QueuedPacket(Packet, SocketAddr);

pub struct WorkStationSender {
    running: Arc<AtomicBool>,
    sock: Arc<UdpSocket>,
    send_queue: Rx<QueuedPacket>
}

impl WorkStationSender {
    pub fn new(running: Arc<AtomicBool>, sock: Arc<UdpSocket>, send_queue: Rx<QueuedPacket>)
        -> Self {
        Self {
            running, sock, send_queue
        }
    }
}

pub fn send_loop(sender: WorkStationSender) -> TResult {
    tokio::spawn(async move {
        while sender.running.load(Ordering::Relaxed) {
            
        }
    });
    Ok(())
}

pub struct WorkStationReceiver {
    running: Arc<AtomicBool>,
    sock: Arc<UdpSocket>,
    recv_queue: Sx<QueuedPacket>
}

impl WorkStationReceiver {
    pub fn new(running: Arc<AtomicBool>, sock: Arc<UdpSocket>, recv_queue: Sx<QueuedPacket>) -> Self {
        Self {
            running, sock, recv_queue
        }
    }
}

pub fn recv_loop(recv: WorkStationReceiver) -> TResult {
    let handle = tokio::spawn(async move {
        while recv.running.load(Ordering::Relaxed) {
            let 
        }
    });
    Ok(())
}

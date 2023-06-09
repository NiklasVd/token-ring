use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, net::SocketAddr};
use crossbeam_channel::{Sender, Receiver};
use tokio::net::UdpSocket;
use crate::{packet::Packet, err::TResult, serialize::Serializer};

pub const RECV_BUF_LENGTH: usize = 1024 * 4;

pub type Sx<T> = Sender<T>;
pub type Rx<T> = Receiver<T>;
pub type Channel<T> = (Sx<T>, Rx<T>);

pub struct QueuedPacket(pub Packet, pub SocketAddr);

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
        loop  {
            while let Ok(next_packet) = sender.send_queue.try_recv() {
                // Catch next packet to be sent from main thread and serialize
                let payload = match next_packet.0.serialize() {
                    Ok(payload) => payload,
                    Err(e) =>  {
                        println!("Send queue encountered serialization error: {e}.");
                        continue
                    },
                };

                // Send packet
                match sender.sock.send_to(
                    payload.as_slice(), next_packet.1).await {
                    Ok(size) => println!("[Send to {:?}] {:?} packet ({size}b).",
                        next_packet.1,
                        next_packet.0.content),
                    Err(e) => {
                        println!("Socket failed to send: {e}.");
                        continue
                    },
                }
            }

            if !sender.running.load(Ordering::Relaxed) {
                break
            }
        }

        println!("Send loop stopped.")
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
        let mut buf = [0u8; RECV_BUF_LENGTH];
        loop {
            // Readability condition required?
            if let Err(e) = recv.sock.readable().await {
                println!("Pending read returned error: {e}.");
                continue
            }

            // Receive new bytes
            let (size, addr) = match recv.sock.try_recv_from(&mut buf) {
                Ok(data) => data,
                Err(e) => {
                    match e.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        _ => println!("Failed to read from socket: {e}."),
                    }
                    continue
                },
            };

            // Slice received bytes from buffer and deserialize
            let recv_buf = &buf[0..size];
            let packet = match Packet::deserialize(recv_buf) {
                Ok(p) => p,
                Err(e) => {
                    println!("Receive queue encountered deserialization error: {e}.");
                    continue
                },
            };
            
            // Pass to main thread
            println!("[Recv from {:?}{:?}] {:?} packet ({size}b).",
                packet.header.val.source, addr, packet.content);
            if let Err(e) = recv.recv_queue.send(QueuedPacket(packet, addr)) {
                println!("Failed to queue received packet: {e}.")
            }

            if !recv.running.load(Ordering::Relaxed) {
                break
            }
        }
        println!("Recv loop stopped.")
    });
    Ok(())
}

use std::{io::Cursor, net::SocketAddr};

use byteorder::{WriteBytesExt, ReadBytesExt};

use crate::{token::Token, id::WorkStationId, serialize::{Serializable, write_sock_addr, write_byte_vec, read_sock_addr, read_byte_vec, get_sock_addr_size}, err::TResult, signature::Signed};

pub struct PacketHeader {
    source: WorkStationId,
}

impl Serializable for PacketHeader {
    type Output = PacketHeader;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.source.write(buf)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(PacketHeader {
            source: WorkStationId::read(buf)?
        })
    }

    fn size(&self) -> usize {
        self.source.size()
    }
}

pub struct Packet {
    header: Signed<PacketHeader>,
    content: PacketType
}

impl Packet {
    fn new(header: Signed<PacketHeader>, content: PacketType) -> Packet {
        Packet {
            header, content
        }
    }
}

impl Serializable for Packet {
    type Output = Packet;
    
    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.header.write(buf)?;
        self.content.write(buf)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let header = Signed::read(buf)?;
        let content = PacketType::read(buf)?;
        Ok(Packet::new(header, content))
    }

    fn size(&self) -> usize {
        self.header.size() + self.content.size()
    }
}

pub enum JoinAnswerResult {
    Confirm(SocketAddr),
    Deny(String)
}

impl Serializable for JoinAnswerResult {
    type Output = JoinAnswerResult;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            JoinAnswerResult::Confirm(target_addr) => {
                buf.write_u8(0)?;
                write_sock_addr(buf, target_addr)
            },
            JoinAnswerResult::Deny(reason) => {
                buf.write_u8(1)?;
                write_byte_vec(buf, &reason.as_bytes().to_vec())
            },
        }?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => JoinAnswerResult::Confirm(read_sock_addr(buf)?),
            1 => JoinAnswerResult::Deny(String::from_utf8(read_byte_vec(buf)?).unwrap()),
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            JoinAnswerResult::Confirm(addr) => get_sock_addr_size(addr),
            JoinAnswerResult::Deny(reason) => reason.len(),
        }
    }
}

pub enum PacketType {
    JoinRequest(WorkStationId),
    JoinAnswer(JoinAnswerResult),
    TokenPass(Token)
}

impl Serializable for PacketType {
    type Output = PacketType;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            PacketType::JoinRequest(id) => {
                buf.write_u8(0)?;
                id.write(buf)
            },
            PacketType::JoinAnswer(result) => {
                buf.write_u8(1)?;
                result.write(buf)
            },
            PacketType::TokenPass(token) => {
                buf.write_u8(2)?;
                token.write(buf)
            },
        }?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => PacketType::JoinRequest(WorkStationId::read(buf)?),
            1 => PacketType::JoinAnswer(JoinAnswerResult::read(buf)?),
            2 => PacketType::TokenPass(Token::read(buf)?),
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            PacketType::JoinRequest(id) => id.size(),
            PacketType::JoinAnswer(result) => result.size(),
            PacketType::TokenPass(token) => token.size(),
        }
    }
}

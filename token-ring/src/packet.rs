use std::{io::Cursor};
use byteorder::{WriteBytesExt, ReadBytesExt};
use crate::{token::Token, id::WorkStationId, serialize::{Serializable, write_byte_vec, read_byte_vec, Serializer, write_string, read_string}, err::TResult, signature::Signed};

/* Packet Layout (in bytes)
    ---------------------------------------------  
    |           Public Key (32b)                | \
    |-------------------------------------------|  |
    |           Signature (64b)                 |  |
    |-------------------------------------------|  | Packet Header (105b total)
    | Packet    |         Source (8b)           |  |
    | Type (1b) |-------------------------------|  |
    |           |         Destination (8b)      | /
    |-------------------------------------------|
    |           Packet Contents                 |
    |                                           |
    |                  ...                      |
    ---------------------------------------------
 */

pub struct PacketHeader {
    pub source: WorkStationId,
    pub destination: WorkStationId
}

impl PacketHeader {
    pub fn new(source: WorkStationId, destination: WorkStationId) -> PacketHeader {
        PacketHeader {
            source, destination
        }
    }
}

impl Serializable for PacketHeader {
    type Output = PacketHeader;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.source.write(buf)?;
        self.destination.write(buf)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let source = WorkStationId::read(buf)?;
        let destination = WorkStationId::read(buf)?;
        Ok(PacketHeader {
            source, destination
        })
    }

    fn size(&self) -> usize {
        self.source.size()
    }
}

pub struct Packet {
    pub header: Signed<PacketHeader>,
    pub content: PacketType
}

impl Packet {
    pub fn new(header: Signed<PacketHeader>, content: PacketType) -> Packet {
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

impl Serializer for Packet {
    fn serialize(&self) -> TResult<Vec<u8>> {
        let mut buf = vec![];
        self.write(&mut buf)?;
        Ok(buf)
    }

    fn deserialize(buf: &[u8]) -> TResult<Self::Output> {
        let mut cursor = Cursor::new(buf);
        let packet = Self::read(&mut cursor)?;
        Ok(packet)
    }
}

#[derive(Debug)]
pub enum JoinAnswerResult {
    Confirm(),
    Deny(String)
}

impl Serializable for JoinAnswerResult {
    type Output = JoinAnswerResult;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            JoinAnswerResult::Confirm() => {
                buf.write_u8(0)?;
                Ok(())
            },
            JoinAnswerResult::Deny(reason) => {
                buf.write_u8(1)?;
                write_byte_vec(buf, &reason.as_bytes().to_vec())
            },
        }?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => JoinAnswerResult::Confirm(),
            1 => JoinAnswerResult::Deny(String::from_utf8(read_byte_vec(buf)?).unwrap()),
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            JoinAnswerResult::Confirm() => 0,
            JoinAnswerResult::Deny(reason) => reason.len(),
        }
    }
}

pub enum PacketType {
    JoinRequest(String),
    JoinReply(JoinAnswerResult),
    TokenPass(Token),
    Leave()
}

impl Serializable for PacketType {
    type Output = PacketType;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            PacketType::JoinRequest(pw) => {
                buf.write_u8(0)?;
                write_string(buf, pw)
            },
            PacketType::JoinReply(result) => {
                buf.write_u8(1)?;
                result.write(buf)
            },
            PacketType::TokenPass(token) => {
                buf.write_u8(2)?;
                token.write(buf)
            },
            PacketType::Leave() => {
                buf.write_u8(3)?;
                Ok(())
            }
        }?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => {
                PacketType::JoinRequest(read_string(buf)?)
            },
            1 => PacketType::JoinReply(JoinAnswerResult::read(buf)?),
            2 => PacketType::TokenPass(Token::read(buf)?),
            3 => PacketType::Leave(),
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            PacketType::JoinRequest(pw) => pw.len(),
            PacketType::JoinReply(result) => result.size(),
            PacketType::TokenPass(token) => token.size(),
            PacketType::Leave() => 0
        }
    }
}

impl std::fmt::Debug for PacketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacketType::JoinRequest(pw) => write!(f, "Join request (pw: {pw})"),
            PacketType::JoinReply(result) => write!(f, "Join reply (result: {:?})", result),
            PacketType::TokenPass(token) => write!(f, "Token pass (token: {:#?})", token),
            PacketType::Leave() => write!(f, "Leave")
        }
    }
}

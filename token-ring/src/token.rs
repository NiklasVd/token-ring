use std::{time::Instant, io::Cursor};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use crate::{id::WorkStationId, serialize::{Serializable, write_instant, read_instant, write_vec, read_vec, write_byte_vec, read_byte_vec}, signature::Signed, err::TResult};

#[derive(Debug, Clone)]
pub struct TokenHeader {
    origin: WorkStationId,
    timestamp: Instant
}

impl TokenHeader {
    pub fn new(origin: WorkStationId) -> TokenHeader {
        TokenHeader {
            origin, timestamp: Instant::now()
        }
    }
}

impl Serializable for TokenHeader {
    type Output = TokenHeader;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.origin.write(buf)?;
        write_instant(buf, self.timestamp)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let origin = WorkStationId::read(buf)?;
        let timestamp = read_instant(buf)?;
        Ok(TokenHeader { origin, timestamp })
    }

    fn size(&self) -> usize {
        self.origin.size() + 4
    }
}

#[derive(Debug, Clone)]
pub enum TokenSendMode {
    Unicast(WorkStationId),
    Broadcast
}

impl Serializable for TokenSendMode {
    type Output = TokenSendMode;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            TokenSendMode::Unicast(dest) => {
                buf.write_u8(0)?;
                dest.write(buf)?;
            },
            TokenSendMode::Broadcast => buf.write_u8(1)?,
        })
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => {
                TokenSendMode::Unicast(WorkStationId::read(buf)?)
            },
            1 => TokenSendMode::Broadcast,
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            TokenSendMode::Unicast(dest) => dest.size(),
            TokenSendMode::Broadcast => 0,
        }
    }
}

#[derive(Debug)]
pub struct TokenFrameId {
    source: WorkStationId,
    timestamp: Instant,
}

impl TokenFrameId {
    pub fn new(source: WorkStationId) -> TokenFrameId {
        TokenFrameId {
            source, timestamp: Instant::now()
        }
    }
}

impl Serializable for TokenFrameId {
    type Output = TokenFrameId;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.source.write(buf)?;
        write_instant(buf, self.timestamp)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let source = WorkStationId::read(buf)?;
        let timestamp = read_instant(buf)?;
        Ok(TokenFrameId {
            source, timestamp
        })
    }

    fn size(&self) -> usize {
        self.source.size() + 4 // Timestamp stored as f32
    }
}

#[derive(Debug)]
pub struct Token {
    header: Signed<TokenHeader>,
    frames: Vec<Signed<TokenFrame>>
}

impl Token {
    pub fn new(header: Signed<TokenHeader>) -> Token {
        Token {
            header, frames: vec![]
        }
    }
}

impl Serializable for Token {
    type Output = Token;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.header.write(buf)?;
        write_vec(buf, &self.frames)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let header = Signed::read(buf)?;
        let frames = read_vec(buf)?;
        Ok(Token {
            header, frames
        })
    }

    fn size(&self) -> usize {
        self.header.size() + self.frames.iter().map(
            |f| f.size()).sum::<usize>()
    }
}

#[derive(Debug)]
pub struct TokenFrame {
    id: TokenFrameId,
    content: TokenFrameType
}

impl TokenFrame {
    pub fn new(id: TokenFrameId, content: TokenFrameType) -> TokenFrame {
        TokenFrame {
            id, content
        }
    }
}

impl Serializable for TokenFrame {
    type Output = TokenFrame;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.id.write(buf)?;
        self.content.write(buf)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let id = TokenFrameId::read(buf)?;
        let content = TokenFrameType::read(buf)?;
        Ok(TokenFrame::new(id, content))
    }

    fn size(&self) -> usize {
        self.id.size() + self.content.size()
    }
}

pub enum TokenFrameType {
    Empty,
    Data {
        destination: WorkStationId,
        send_mode: TokenSendMode,
        seq: u16, // Sequence of frame (for identification purposes)
        payload: Vec<u8>
    },
    DataReceived {
        source: WorkStationId,
        seq: u16
    }
}

impl Serializable for TokenFrameType {
    type Output = TokenFrameType;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        Ok(match self {
            TokenFrameType::Empty => buf.write_u8(0)?,
            TokenFrameType::Data { destination,
                send_mode, seq, payload } => {
                buf.write_u8(1)?;

                destination.write(buf)?;
                send_mode.write(buf)?;
                buf.write_u16::<BigEndian>(*seq)?;
                write_byte_vec(buf, payload)?;
            },
            TokenFrameType::DataReceived { source, seq } => {
                buf.write_u8(2)?;

                source.write(buf)?;
                buf.write_u16::<BigEndian>(*seq)?;
            },
        })
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(match buf.read_u8()? {
            0 => TokenFrameType::Empty,
            1 => {
                let destination = WorkStationId::read(buf)?;
                let send_mode = TokenSendMode::read(buf)?;
                let seq = buf.read_u16::<BigEndian>()?;
                let payload = read_byte_vec(buf)?;
                TokenFrameType::Data { destination, send_mode, seq, payload }
            },
            2 => {
                let source = WorkStationId::read(buf)?;
                let seq = buf.read_u16::<BigEndian>()?;
                TokenFrameType::DataReceived { source, seq }
            },
            n @ _ => panic!("Index out of bounds: {n}.")
        })
    }

    fn size(&self) -> usize {
        1 + match self {
            TokenFrameType::Empty => 0,
            TokenFrameType::Data { destination, send_mode,
                payload, .. } =>
                destination.size() + send_mode.size() + 2 + payload.len(),
            TokenFrameType::DataReceived { source, .. } => 
                source.size() + 2,
        }
    }
}

impl std::fmt::Debug for TokenFrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenFrameType::Empty => write!(f, "Token frame empty"),
            TokenFrameType::Data { destination,
                send_mode, seq, payload } => 
                write!(f, "Token frame data (to: {destination}, delivery: {:?}, seq: {seq}, payload size: {:?}", send_mode, payload.len()),
            TokenFrameType::DataReceived { source, seq } => 
                write!(f, "Token frame data ack (from: {source}, seq: {seq})"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn serialize() {
        
    }
}

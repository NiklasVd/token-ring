use core::fmt;
use std::{io::Cursor};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};
use crate::{id::WorkStationId, serialize::{Serializable, write_vec, read_vec, write_byte_vec, read_byte_vec}, signature::Signed, err::TResult, util::timestamp};

#[derive(Debug, Clone, PartialEq)]
pub struct TokenHeader {
    origin: WorkStationId,
    timestamp: u64
}

impl TokenHeader {
    pub fn new(origin: WorkStationId) -> TokenHeader {
        TokenHeader {
            origin, timestamp: timestamp()
        }
    }
}

impl Serializable for TokenHeader {
    type Output = TokenHeader;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.origin.write(buf)?;
        Ok(buf.write_u64::<BigEndian>(self.timestamp)?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let origin = WorkStationId::read(buf)?;
        let timestamp = buf.read_u64::<BigEndian>()?;
        Ok(TokenHeader { origin, timestamp })
    }

    fn size(&self) -> usize {
        self.origin.size() + 4
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenFrameId {
    pub source: WorkStationId,
    timestamp: u64,
}

impl TokenFrameId {
    pub fn new(source: WorkStationId) -> TokenFrameId {
        TokenFrameId {
            source, timestamp: timestamp()
        }
    }
}

impl Serializable for TokenFrameId {
    type Output = TokenFrameId;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        self.source.write(buf)?;
        Ok(buf.write_u64::<BigEndian>(self.timestamp)?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let source = WorkStationId::read(buf)?;
        let timestamp = buf.read_u64::<BigEndian>()?;
        Ok(TokenFrameId {
            source, timestamp
        })
    }

    fn size(&self) -> usize {
        self.source.size() + 4 // Timestamp stored as f32
    }
}

#[derive(Clone, PartialEq)]
pub struct Token {
    pub header: Signed<TokenHeader>,
    // Signed container not necessary anymore
    // Using star topology now, so active monitor (de facto server) will 
    // be able to check validity of token changes by each client after they pass it on.
    pub frames: Vec<TokenFrame>
}

impl Token {
    pub fn new(header: Signed<TokenHeader>) -> Token {
        Token {
            header, frames: vec![]
        }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Origin: {:?}, Frames: {:?} ", self.header.val.origin, self.frames)
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

#[derive(Clone, PartialEq, Eq)]
pub struct TokenFrame {
    pub id: TokenFrameId,
    pub content: TokenFrameType
}

impl TokenFrame {
    pub fn new(id: TokenFrameId, content: TokenFrameType) -> TokenFrame {
        TokenFrame {
            id, content
        }
    }
}

impl fmt::Debug for TokenFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Frame{:?}{} {:?}", self.id.source, self.id.timestamp, self.content)
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

#[derive(Clone, PartialEq, Eq)]
pub enum TokenFrameType {
    Empty,
    Data {
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
            TokenFrameType::Data { send_mode,
                seq, payload } => {
                buf.write_u8(1)?;

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
                let send_mode = TokenSendMode::read(buf)?;
                let seq = buf.read_u16::<BigEndian>()?;
                let payload = read_byte_vec(buf)?;
                TokenFrameType::Data { send_mode, seq, payload }
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
            TokenFrameType::Data { send_mode,
                payload, .. } =>
                send_mode.size() + 2 + payload.len(),
            TokenFrameType::DataReceived { source, .. } => 
                source.size() + 2,
        }
    }
}

impl std::fmt::Debug for TokenFrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenFrameType::Empty => write!(f, "Empty"),
            TokenFrameType::Data { send_mode,
                payload, .. } => 
                write!(f, "Data: {:?}, {:?}b", send_mode, payload.len()),
            TokenFrameType::DataReceived { source, .. } => 
                write!(f, "Data Ack: {source}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use crate::{signature::{generate_keypair, Signed}, id::WorkStationId, serialize::Serializable};
    use super::{Token, TokenHeader, TokenFrame, TokenFrameId, TokenSendMode, TokenFrameType};

    fn create_token_stub() -> Token {
        let keypair = generate_keypair();
        let header = TokenHeader::new(
            WorkStationId::new("Test".to_owned()));
        let signed_header = Signed::new(&keypair, header).unwrap();
        let mut token = Token::new(signed_header);
        let frame = TokenFrame::new(TokenFrameId::new(
        WorkStationId::new("Some Station".to_owned())),
        TokenFrameType::Data { send_mode: TokenSendMode::Broadcast,
            seq: 0, payload: vec![0, 1, 2] });
        token.frames.push(frame);
        token
    }

    #[test]
    fn serialize() {
        let token = create_token_stub();       
        let mut buf = vec![];
        token.write(&mut buf).unwrap();
    }

    #[test]
    fn deserialize() {
        let token = create_token_stub();       
        let mut buf = vec![];
        assert!(token.write(&mut buf).is_ok());

        let mut cursor = Cursor::new(buf.as_slice());
        let new_token = Token::read(&mut cursor).unwrap();
        
        assert_eq!(token, new_token)
    }
}

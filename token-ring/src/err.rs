use core::fmt;
use std::error::Error;
use crossbeam_channel::{SendError, RecvError};
use ed25519_dalek::SignatureError;

use crate::comm::QueuedPacket;

pub type TResult<T = ()> = Result<T, GlobalError>;

pub enum GlobalError {
    Internal(TokenRingError),
    Io(std::io::Error),
    Signature(SignatureError),
    CrossbeamSend(SendError<QueuedPacket>),
    CrossbeamRecv(RecvError),
    Unknown
}

impl fmt::Debug for GlobalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobalError::Internal(err) => write!(f, "{err}"),
            GlobalError::Io(err) => write!(f, "{err}"),
            GlobalError::Signature(err) => write!(f, "{err}"),
            GlobalError::CrossbeamSend(err) => write!(f, "{err}"),
            GlobalError::CrossbeamRecv(err) => write!(f, "{err}"),
            GlobalError::Unknown => write!(f, "Unknown error occured!"),
        }
    }
}

impl fmt::Display for GlobalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self) // TODO: Implement proper display print
    }
}

impl Error for GlobalError {
    
}

// --- Implicit conversions ---

impl From<std::io::Error> for GlobalError {
    fn from(value: std::io::Error) -> Self {
        GlobalError::Io(value)
    }
}

impl From<SignatureError> for GlobalError {
    fn from(value: SignatureError) -> Self {
        GlobalError::Signature(value)
    }
}

impl From<SendError<QueuedPacket>> for GlobalError {
    fn from(value: SendError<QueuedPacket>) -> Self {
        GlobalError::CrossbeamSend(value)
    }
}

impl From<RecvError> for GlobalError {
    fn from(value: RecvError) -> Self {
        GlobalError::CrossbeamRecv(value)
    }
}

// ---

#[derive(Debug, Clone, Copy)]
pub enum TokenRingError {
    InvalidPacketHeader,
    NotConnected,
    Unknown
}

impl fmt::Display for TokenRingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self) // TODO: Implement proper display print
    }
}

impl Error for TokenRingError {
}

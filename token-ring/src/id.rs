use core::fmt;
use std::io::Cursor;
use crate::{serialize::{Serializable, write_byte_vec, read_byte_vec}, err::TResult};

#[derive(Clone, PartialEq)]
pub struct WorkStationId {
    name: String
}

impl WorkStationId {
    pub fn new(name: String) -> WorkStationId {
        WorkStationId {
            name: name.to_ascii_lowercase()
        }
    }
}

impl Serializable for WorkStationId {
    type Output = WorkStationId;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        write_byte_vec(buf, &self.name.as_bytes().to_vec())
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(WorkStationId::new(
            String::from_utf8(read_byte_vec(buf)?).unwrap() /* Check err... */))
    }

    fn size(&self) -> usize {
        self.name.len() // Assumes ASCII
    }
}

impl fmt::Debug for WorkStationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}/", self.name)
    }
}

impl fmt::Display for WorkStationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}/", self.name)
    }
}

use core::fmt;
use std::{io::Cursor, time::SystemTime};
use crate::{serialize::{Serializable, write_string, read_string}, err::TResult};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WorkStationId {
    // Max size 8 chars
    name: String,
    num: u16
}

impl WorkStationId {
    pub fn new(mut name: String) -> WorkStationId {
        if name.len() > 8 {
            name.truncate(8);
        }
        let num = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u16;

        WorkStationId {
            name, num
        }
    }
}

impl Serializable for WorkStationId {
    type Output = WorkStationId;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        write_string(buf, &self.name)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        Ok(WorkStationId::new(read_string(buf)?))
    }

    fn size(&self) -> usize {
        self.name.len() // Assumes ASCII
    }
}

impl fmt::Debug for WorkStationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}{}/", self.name, self.num)
    }
}

impl fmt::Display for WorkStationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.name, self.num)
    }
}

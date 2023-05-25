use core::fmt;
use std::{io::Cursor, time::SystemTime};
use byteorder::{WriteBytesExt, BigEndian, ReadBytesExt};

use crate::{serialize::{Serializable, write_string, read_string}, err::TResult};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WorkStationId {
    // Max size 8 chars
    name: String
}

impl WorkStationId {
    pub fn new(mut name: String) -> WorkStationId {
        if name.len() > 8 {
            name.truncate(8);
        }
        // let num = SystemTime::now()
        //     .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u16;

        WorkStationId {
            name
        }
    }
}

impl Serializable for WorkStationId {
    type Output = WorkStationId;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        write_string(buf, &self.name)
        //Ok(buf.write_u16::<BigEndian>(self.num)?)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let name = read_string(buf)?;
        //let num = buf.read_u16::<BigEndian>()?;
        Ok(WorkStationId {
            name
        })
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
        write!(f, "{}", self.name)
    }
}

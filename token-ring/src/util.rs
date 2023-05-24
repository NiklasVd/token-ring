use std::time::{UNIX_EPOCH, SystemTime};

pub fn timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
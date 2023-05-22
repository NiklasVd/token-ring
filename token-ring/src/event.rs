use crate::{id::WorkStationId, packet::JoinAnswerResult};

pub trait Event {
    fn source(&self) -> &WorkStationId;
}

pub struct JoinAnswerEvent {
    pub source: WorkStationId,
    pub result: JoinAnswerResult
}

impl Event for JoinAnswerEvent {
    fn source(&self) -> &WorkStationId {
        &self.source
    }
}

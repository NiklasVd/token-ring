use std::{collections::HashMap, time::Instant};
use crate::{id::WorkStationId, token::Token, err::{TResult, TokenRingError, GlobalError}};

pub struct StationStatus(pub bool /* Received token this round? */, /* u32 (Checksum?) */);

pub enum TokenPassMode {
    Idle, // Token sending paused or not enough stations connected
    Passed, // Token passed to station (waiting for timeout or retrieval)
    Received, // Token has been received by station and can be passed on
}

pub struct TokenState(pub WorkStationId /* Sent to */, pub Instant /* Sent when */);

pub struct TokenPasser {
    pub curr_token: Option<Token>,
    state: Option<TokenState>,
    pass_mode: TokenPassMode,
    max_passover_time: f32,
    // List with all connected stations, sets the order in which passive stations
    // receive token and stores if they were owned one in current rotation.
    // TODO: Set order of stations! Hash maps are not ordered, hence the token will
    // be passed randomly between stations.
    pub station_status: HashMap<WorkStationId, StationStatus>,
}

impl TokenPasser {
    pub fn new(max_passover_time: f32) -> TokenPasser {
        TokenPasser {
            curr_token: None, state: None, pass_mode: TokenPassMode::Idle,
            max_passover_time, station_status: HashMap::new()
        }
    }

    pub fn pass_ready(&mut self) -> bool {
        if let Some(TokenState(
            _, send_time)) = self.state.as_mut() {
            match self.pass_mode {
                TokenPassMode::Received => {
                    true
                },
                _ => {
                    if Instant::now().duration_since(*send_time)
                        .as_secs_f32() >= self.max_passover_time {
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            true
        }
    }

    pub fn recv_token(&mut self, new_token: Token, sender_id: &WorkStationId) -> TResult {
        if let Some(TokenState(
            id, send_time)) = self.state.as_mut() {
            let total_pass_time = Instant::now().duration_since(*send_time).as_secs_f32();
            // Has station overstepped the time limit?
            if total_pass_time <= self.max_passover_time {
                // Is token header valid (i.e., is it actually from the active station)?
                if new_token.header.verify() {
                    // Is the sender of the token actually the expected sender currently registered?
                    if sender_id == id {
                        // Check validity of token frames
                        // (Has the current token holder only modified accessible frames?)
                        
                        // If all is good, set pass mode to Received and get ready for next pass
                        if let Some(status) = self.get_station(sender_id) {
                            if status.0 {
                                println!("Station {sender_id} that currently holds token already held it before this rotation.");
                            }
                            // Set station status to true now that token was passed to it.
                            status.0 = true;
                            // Update new token
                            self.curr_token = Some(new_token);
                            // Set pass mode so that new token may be sent
                            self.pass_mode = TokenPassMode::Received;
                            println!("Received valid token from {sender_id}. Ready to pass on.");
                            return Ok(())
                        }
                    } else {
                        println!("Received token from wrong station: {sender_id}. Expecting: {id}. Discarding.");
                    }
                } else {
                    println!("Received invalid token header from {sender_id}. Discarding.");
                }
            } else {
                println!("Received token too late ({total_pass_time}s) from {sender_id}. Discarding.");
            }
        } else {
            println!("Token sent by {sender_id}. Did not expect token.");
        }
        Err(GlobalError::Internal(TokenRingError::InvalidToken(sender_id.clone(), new_token)))
    }

    pub fn pass_token(&mut self, to_id: WorkStationId) {
        self.state = Some(TokenState(to_id, Instant::now()));
        self.pass_mode = TokenPassMode::Passed;
    }

    pub fn select_next_station(&mut self) -> Option<WorkStationId> {
        if self.station_status.len() == 0 {
            return None
        }

        let next_station = if let Some((next_station_id, _)) = self.station_status.iter()
            .find(|(_, status)| !status.0) {
            next_station_id.clone()
        } else {
            // This token rotation is over. Reset status of all stations and send
            // new token.
            self.station_status.values_mut().for_each(|s| s.0 = false);
            self.station_status.keys().last().unwrap().clone()
        };
        self.pass_token(next_station.clone());
        Some(next_station)
    }

    fn get_station(&mut self, id: &WorkStationId) -> Option<&mut StationStatus> {
        self.station_status.get_mut(&id)
    }
}

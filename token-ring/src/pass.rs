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
                        println!("Current token holder took too long for token pass.");
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
        if let Some(status) = self.get_station(sender_id) {
            // Whether or not token is valid, this station is ticked off the list.
            status.0 = true;
            self.pass_mode = TokenPassMode::Received;

            match self.check_token_validity(&new_token, sender_id) {
                Ok(()) => {
                    // Update new token
                    self.curr_token = Some(new_token);
                    // Set pass mode so that new token may be sent
                    
                    println!("Received valid token from {sender_id}. Ready to pass on.");
                    Ok(())
                },
                Err(e) => Err(e)
            }
        } else {
            println!("Token sender is not part of registered station list. Ignoring.");
            Err(GlobalError::Internal(TokenRingError::InvalidToken(sender_id.clone(), new_token)))
        }
    }

    fn check_token_validity(&self, token: &Token, sender_id: &WorkStationId) -> TResult {
        if let Some(TokenState(
            id, send_time)) = self.state.as_ref() {
            let total_pass_time = Instant::now().duration_since(*send_time).as_secs_f32();
            // Has station overstepped the time limit?
            if total_pass_time <= self.max_passover_time {
                // Is token header valid (i.e., is it actually from the active station)?
                if token.header.verify() {
                    // Is the sender of the token actually the expected sender currently registered?
                    if sender_id == id {
                        return Ok(())
                    } else {
                        println!("Received token from wrong station: {sender_id}. Expecting: {id}. Discarding.");
                    }
                } else {
                    println!("Received invalid token header from {sender_id}. Discarding.");
                }
            } else {
                println!("Received token too late ({total_pass_time}s) from {sender_id}. Discarding.");
            }
        }
        Err(GlobalError::Internal(TokenRingError::InvalidToken(sender_id.clone(), token.clone())))
    }

    pub fn pass_token(&mut self, to_id: WorkStationId) {
        self.state = Some(TokenState(to_id, Instant::now()));
        self.pass_mode = TokenPassMode::Passed;
    }

    pub fn select_next_station(&mut self) -> Option<WorkStationId> {
        if self.station_status.len() == 0 {
            return None
        }

        // If there are stations on the list that didn't yet hold the token, send there.
        let next_station = if let Some((next_station_id, _)) = self.station_status.iter()
            .find(|(_, status)| !status.0) {
            next_station_id.clone()
        } else {
            // This token rotation is over. Reset status of all stations and send
            // new token.
            let mut station_order = vec![];
            self.station_status.iter_mut().for_each(|(id, status)| {
                status.0 = false;
                station_order.push(id);
            });

            println!("Token passing order:");
            for s_o in station_order.into_iter() {
                print!("->{s_o}");
            }
            println!(".");
            
            // Select the next station to hold the new token (here: last station in hashmap)
            self.station_status.keys().last().unwrap().clone()
        };

        self.pass_token(next_station.clone());
        Some(next_station)
    }

    fn get_station(&mut self, id: &WorkStationId) -> Option<&mut StationStatus> {
        self.station_status.get_mut(&id)
    }
}

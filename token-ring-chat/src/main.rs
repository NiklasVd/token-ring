use std::{io::{stdin, stdout, Write, Cursor}, net::{SocketAddr}, str::FromStr, fmt::Debug};
use token_ring::{station::PassiveStation, err::TResult, id::WorkStationId, token::{TokenFrameType, TokenSendMode}, serialize::{read_string, write_string}};

#[tokio::main]
async fn main() -> TResult {
    println!("Token Ring Chat Node");

    let name = read_line("Enter ID (max 8 chars ASCII)");
    let port = read::<u16>("Listen on port");
    let mut passive_station = PassiveStation::new(
        WorkStationId::new(name), port).await?;
    println!("Setup passive station.");

    println!("Ready to connect to active station.");
    let target_addr = read::<SocketAddr>("Enter socket addr");
    let pw = read_line("Enter password");
    passive_station.connect(target_addr, pw).await?;
    loop {
        match passive_station.recv_next().await {
            Ok(_) => {
                if let Some(curr_token) = passive_station.get_token_mut() {
                    for frame in curr_token.frames.iter() {
                        match &frame.content {
                            TokenFrameType::Data {
                                send_mode, seq, payload } => {
                                    let mut cursor = Cursor::new(payload.as_slice());
                                    let text = token_ring::serialize::read_string(&mut cursor)?;
                                    println!("{:?} wrote: {text}.", frame.id.source);
                                },
                                _ => ()
                        }
                    }
                    let text = format!("Some text.");
                    let mut buf = vec![];
                    write_string(&mut buf, &text)?;
                    passive_station.append_frame(TokenFrameType::Data {
                        send_mode: TokenSendMode::Broadcast, seq: 0, payload: buf });
                    
                    passive_station.pass_on_token()?;
                }
            },
            Err(e) => println!("Recv err: {e}."),
        }


        // let text = read_line("Write");
        // if !text.is_empty() {
        //     let mut buf = vec![];
        //     match write_string(&mut buf, &text) {
        //         Ok(()) => (),
        //         Err(e) => {
        //             println!("Invalid chat message: {e}.");
        //         }
        //     };
        //     passive_station.append_frame(TokenFrameType::Data {
        //         send_mode: TokenSendMode::Broadcast, seq: 0, payload: buf })
        // }
        stdout().flush().unwrap();
    }
}

pub fn read_line(input: &str) -> String {
    let mut line = String::new();
    print!("{}/: ", input);
    stdout().flush().expect("Failed to flush stream");

    stdin().read_line(&mut line).expect("Failed to read line");
    line.trim().to_owned()
}

pub fn read<T: FromStr + Debug>(input: &str) -> T where <T as FromStr>::Err: Debug {
    match read_line(input.clone()).parse::<T>() {
        Ok(n) => n,
        Err(e) => {
            println!("{:?}", e);
            read(input)
        }
    }
}

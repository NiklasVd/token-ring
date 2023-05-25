use std::{io::{stdin, stdout, Write}, net::{SocketAddr}, str::FromStr, fmt::Debug};
use log::{info, warn};
use token_ring::{station::PassiveStation, err::TResult, id::WorkStationId};

#[tokio::main]
async fn main() -> TResult {
    println!("Token Ring Chat Client");

    let name = read_string("Enter ID (max 8 chars ASCII)");
    let port = read::<u16>("Listen on port");
    let mut passive_station = PassiveStation::new(
        WorkStationId::new(name), port).await?;
    info!("Setup passive station.");

    info!("Ready to connect to active station.");
    let target_addr = read::<SocketAddr>("Enter socket addr");
    let pw = read_string("Enter password");
    passive_station.connect(target_addr, pw).await?;
    loop {
        match passive_station.recv_next().await {
            Ok(_) => {
                if let Some(curr_token) = passive_station.get_token_mut() {
                    info!("Held token! Yayy");
                    passive_station.pass_on_token()?;
                }
            },
            Err(e) => warn!("Recv err: {e}."),
        }
    }

    Ok(())
}

pub fn read_string(input: &str) -> String {
    let mut line = String::new();
    print!("{}~ ", input);
    stdout().flush().expect("Failed to flush stream");

    stdin().read_line(&mut line).expect("Failed to read line");
    line.trim().to_owned()
}

// pub fn read_u16(input: &str) -> u16 {
//     match read_string(input.clone()).parse::<u16>() {
//         Ok(n) => n,
//         Err(e) => {
//             println!("{}", e);
//             read_u16(input)
//         }
//     }
// }

pub fn read<T: FromStr + Debug>(input: &str) -> T where <T as FromStr>::Err: Debug {
    match read_string(input.clone()).parse::<T>() {
        Ok(n) => n,
        Err(e) => {
            println!("{:?}", e);
            read(input)
        }
    }
}

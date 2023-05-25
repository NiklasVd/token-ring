use std::{io::{stdout, stdin, Write}, fmt::Debug, str::FromStr, time::Duration};
use token_ring::{station::{ActiveStation, GlobalConfig}, id::WorkStationId, err::TResult};

#[tokio::main]
async fn main() -> TResult {
    println!("Token Ring Chat Auth");

    let name = read_string("Enter ID (max 8 chars ASCII)");
    let port = read::<u16>("Listen on port");
    let pw = read_string("Enter password (optional)");
    let mut active_station = ActiveStation::host(
        WorkStationId::new(name), GlobalConfig::new(
            pw, true, 32, 5.),
        port).await?;
    println!("Hosting active station.");

    loop {
        match active_station.recv_all().await {
            Ok(_) => (),
            Err(e) => println!("Recv err: {e}.")
        }
        match active_station.poll_token_pass().await {
            Ok(()) => (),
            Err(e) => println!("Token poll err: {e}.")
        }
        tokio::time::sleep(Duration::from_secs_f32(2.5)).await;
        stdout().flush().unwrap();
    }
}

pub fn read_string(input: &str) -> String {
    let mut line = String::new();
    print!("{}/: ", input);
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


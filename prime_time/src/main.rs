use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080")?;

    for stream in listener.incoming() {
        thread::spawn(move || {
            eprintln!("Opening connection");

            let mut stream = stream.expect("open incoming TCP stream");
            let mut buf = Vec::new();

            while let Ok(bytes_read) = stream.read_to_end(&mut buf) {
                eprintln!("Got {bytes_read} bytes: {buf:?}");

                let json = serde_json::from_slice::<PrimeRequest>(&buf);
            }

            eprintln!("Closing connection");
        });
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct PrimeRequest {
    method: String,
    number: serde_json::Number,
}

#[derive(serde::Serialize)]
struct PrimeResponse {
    method: &'static str,
    prime: bool,
}

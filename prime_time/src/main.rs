use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpListener;
use std::thread;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080")?;

    for stream in listener.incoming() {
        thread::spawn(move || {
            eprintln!("Opening connection");

            let mut stream = stream.expect("opened TCP stream");
            let mut reader = BufReader::new(stream.try_clone().expect("cloned stream"));
            let mut buf = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut buf) {
                if bytes_read == 0 {
                    eprintln!("No bytes read");
                    break;
                }

                eprintln!("Got {bytes_read} bytes: {buf:?}");
                eprintln!("{buf}");

                let json: serde_json::Value =
                    match serde_json::from_str::<PrimeRequest>(buf.trim_end()) {
                        Ok(data) if data.method == "isPrime" => {
                            eprintln!("Struct: {data:?}");
                            serde_json::json!({
                                "method": "isPrime",
                                "prime": primes::is_prime(data.number.as_u64().unwrap_or_default())
                            })
                        }
                        _ => {
                            eprintln!("Bad request: {buf:?}");
                            serde_json::json!("malformed")
                        }
                    };

                let res_str = serde_json::to_string(&json).expect("infallible");
                writeln!(stream, "{res_str}").ok();
                buf.clear();
            }

            eprintln!("Closing connection");
        });
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct PrimeRequest {
    method: String,
    number: serde_json::Number,
}

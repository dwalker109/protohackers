use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080")?;

    for stream in listener.incoming() {
        thread::spawn(move || {
            eprintln!("Opening connection");

            let mut stream = stream.expect("incoming TCP stream");
            let mut buf = [0u8; 16];

            while let Ok(r) = stream.read(&mut buf) {
                if r == 0 {
                    eprintln!("No bytes to echo");
                    break;
                } else {
                    eprintln!("Writing {r} bytes to stream");
                    let _w = stream.write_all(&buf);
                    buf = [0; 16];
                }
            }

            eprintln!("Closing connection");
        });
    }

    Ok(())
}

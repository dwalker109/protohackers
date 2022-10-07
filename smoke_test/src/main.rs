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
                    eprintln!("Bytes: {buf:?}");
                    let r_str = String::from_utf8_lossy(&buf);
                    eprintln!("As String: {r_str}");
                    let _w = stream.write_all(&buf[..r]);
                }
            }

            eprintln!("Closing connection");
        });
    }

    Ok(())
}

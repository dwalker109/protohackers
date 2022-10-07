use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080")?;

    for stream in listener.incoming() {
        eprintln!("Incoming connection started");

        thread::spawn(move || {
            let mut stream = stream.expect("incoming TCP stream");
            let mut buf = [0u8; 16];

            loop {
                eprint!("Reading stream");
                let _r = stream.read(&mut buf);
                eprint!("Writing stream");
                let _w = stream.write_all(&buf);
                buf = [0u8; 16];
            }
        });
    }

    Ok(())
}

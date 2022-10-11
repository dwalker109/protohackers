use std::{collections::HashMap, net::UdpSocket};

static V: (&str, &str) = ("version", "Dan's KV Store 0.1");
static DELIM: char = '=';

fn main() {
    let socket =
        UdpSocket::bind("0.0.0.0:8080").expect("opened UDP socket on port 8080 on 0.0.0.0");

    eprintln!("Opened socket");

    let mut db = HashMap::new();
    db.insert(V.0.to_string(), V.1.to_string());

    let mut buf = [0x00u8; 1000];

    while let Ok((bytes_read, src)) = socket.recv_from(&mut buf) {
        let req = String::from_utf8_lossy(&buf[..bytes_read]);

        match req.split_once(DELIM) {
            Some((k, _)) if k == V.0 => {
                eprintln!("Ignoring attempt to set version");
            }
            Some((k, v)) => {
                eprintln!("Setting {k}={v}");
                db.entry(k.to_string())
                    .and_modify(|e| *e = v.to_string())
                    .or_insert_with(|| v.to_string());
                eprintln!("Done");
            }
            None => {
                eprintln!("Getting {req}");
                let v = db
                    .get(&req.to_string())
                    .map_or_else(|| &[], String::as_bytes);
                socket
                    .send_to(&[req.as_bytes(), &[b'='], v].concat(), src)
                    .ok();
                eprintln!("Done");
            }
        }
    }

    eprintln!("Closing socket");
}

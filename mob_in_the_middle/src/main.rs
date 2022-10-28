use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("bound port 80 on 0.0.0.0");

    for client_stream in listener.incoming() {
        thread::spawn(move || {
            let client_stream = client_stream.unwrap();
            let mut client_writer = client_stream.try_clone().unwrap();
            let mut client_reader = BufReader::new(client_writer.try_clone().unwrap());

            let chatsrv_stream = TcpStream::connect("chat.protohackers.com:16963").unwrap();
            let mut chatsrv_writer = chatsrv_stream.try_clone().unwrap();
            let mut chatsrv_reader = BufReader::new(chatsrv_stream.try_clone().unwrap());

            let (chatsrv_ended, init_shutdown) = mpsc::channel::<()>();
            let client_ended = chatsrv_ended.clone();

            thread::spawn(move || {
                let mut msg = String::new();
                while let Ok(n) = chatsrv_reader.read_line(&mut msg) {
                    if n == 0 {
                        eprintln!("Server EOF");
                        break;
                    }

                    client_writer.write_all(tamper_msg(&msg).as_bytes()).ok();
                    msg.clear();
                }
                chatsrv_ended.send(()).ok();
            });

            thread::spawn(move || {
                let mut msg = String::new();
                while let Ok(n) = client_reader.read_line(&mut msg) {
                    if n == 0 {
                        eprintln!("Client EOF");
                        break;
                    }

                    chatsrv_writer.write_all(tamper_msg(&msg).as_bytes()).ok();
                    msg.clear();
                }
                client_ended.send(()).ok();
            });

            let h = thread::spawn(move || {
                let _ = init_shutdown.recv();
                chatsrv_stream.shutdown(Shutdown::Both).ok();
                client_stream.shutdown(Shutdown::Both).ok();
            });

            h.join().ok();
        });
    }
}

static TONY_BC_ADDR: &str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";
static BC_ADDR_REGEXP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?P<pre>^| )(?P<addr>7[A-Za-z0-9]{24,35})(?P<post>\n| )"#).unwrap());
static BC_ADDR_REPLACEMENT: Lazy<String> = Lazy::new(|| format!("${{pre}}{TONY_BC_ADDR}${{post}}"));

fn tamper_msg(msg: &str) -> String {
    msg.split_inclusive(' ')
        .map(|s| BC_ADDR_REGEXP.replace(s, BC_ADDR_REPLACEMENT.as_str()))
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single() {
        let r = tamper_msg("7iKDZEwPZSqIvDnHvVN2r0hUWXD5rHX\n");
        assert_eq!(r.as_str(), format!("{TONY_BC_ADDR}\n"));
    }

    #[test]
    fn multiple() {
        let r = tamper_msg("7iKDZEwPZSqIvDnHvVN2r0hUWXD5rHX 7iKDZEwPZSqIvDnHvVN2r0hUWXD5rHX 7iKDZEwPZSqIvDnHvVN2r0hUWXD5rHX \n");
        assert_eq!(
            r.as_str(),
            format!("{TONY_BC_ADDR} {TONY_BC_ADDR} {TONY_BC_ADDR} \n")
        );
    }

    #[test]
    fn not_actually_bc() {
        let orig = "7iKDZEwPZSqIvDnHvVN2r0hUWXD5rHX-123456789\n";
        let r = tamper_msg(orig);
        assert_eq!(r.as_str(), orig);
    }
}

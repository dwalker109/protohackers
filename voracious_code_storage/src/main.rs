use std::{
    io::{BufRead, BufReader, BufWriter, Read, Write},
    net::TcpListener,
    thread,
};

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    let repo = repo::Repo::new();

    for stream in listener.incoming() {
        let mut repo = repo.clone();

        thread::spawn(move || {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = BufWriter::new(stream);
            let mut buf_op = String::with_capacity(512);
            let mut buf_dat = vec![0; 1024];

            loop {
                writer.write_all(b"READY\n").ok();
                writer.flush().ok();

                buf_op.clear();
                if let Ok(0) = reader.read_line(&mut buf_op) {
                    break;
                }

                match parsers::parse(buf_op.as_bytes()) {
                    Ok((_, op)) => match op {
                        parsers::Op::Put(path, len) => {
                            buf_dat.resize(len, 0);
                            reader.read_exact(&mut buf_dat).ok();

                            let res = match repo.put(&path, &buf_dat) {
                                Ok(rev) => format!("OK r{rev}\n"),
                                Err(err) => err.to_string(),
                            };

                            writer.write_all(res.as_bytes()).ok();
                        }
                        parsers::Op::Get(path, rev) => {
                            let res = match repo.get(&path, rev) {
                                Ok((_rev, data)) => {
                                    let len = data.len();
                                    format!("OK {len}\n{data}")
                                }
                                Err(err) => err.to_string(),
                            };

                            writer.write_all(res.as_bytes()).ok();
                        }
                        parsers::Op::List(path) => {
                            let list = repo.list(&path);

                            writer
                                .write_all(format!("OK {}\n", list.len()).as_bytes())
                                .ok();

                            for i in list {
                                writer.write_all(i.to_string().as_bytes()).ok();
                            }
                        }
                        parsers::Op::Help => {
                            writer
                                .write_all("OK usage: HELP|GET|PUT|LIST".as_bytes())
                                .ok();
                        }
                        parsers::Op::Err(err) => {
                            writer.write_all(err.to_string().as_bytes()).ok();
                        }
                    },
                    Err(_) => {
                        writer.write_all(b"ERR\n").ok();
                    }
                }

                writer.flush().ok();

                eprintln!("Tracking {} files...", repo.len())
            }
        });
    }
}

mod error;
mod parsers;
mod repo;

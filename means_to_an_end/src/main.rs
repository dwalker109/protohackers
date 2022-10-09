use std::{
    io::{prelude::*, BufReader},
    net::TcpListener,
    thread,
};

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("bound port 80 on 0.0.0.0");

    for stream in listener.incoming() {
        thread::spawn(move || {
            eprintln!("Opening connection");

            let mut db = Vec::new();

            let mut stream = stream.expect("opened TCP stream");
            let mut reader = BufReader::new(stream.try_clone().expect("cloned stream"));
            let mut buf = [0x00; 9];

            while reader.read_exact(&mut buf).is_ok() {
                match buf[0] {
                    b'I' => db.push(Insert::from(&buf)),
                    b'Q' => {
                        let q = Query::from(&buf);

                        let matches = db
                            .iter()
                            .filter_map(|i| match (q.mintime..=q.maxtime).contains(&i.timestamp) {
                                true => Some(i64::from(i.price)),
                                false => None,
                            })
                            .collect::<Vec<_>>();

                        let mean = i32::try_from(
                            matches.iter().sum::<i64>() / std::cmp::max(1, matches.len()) as i64,
                        )
                        .expect("mean val fits into an i32");

                        stream.write(&mean.to_be_bytes()).ok();
                    }
                    _ => {
                        eprintln!("Skipping {buf:?}");
                        return;
                    }
                }
            }

            eprintln!("Closing connection");
        });
    }
}

type MessageBytes = [u8; 9];

struct Message(i32, i32);

impl From<&MessageBytes> for Message {
    fn from(b: &MessageBytes) -> Self {
        Self(
            i32::from_be_bytes(b[1..5].try_into().expect("infallible")),
            i32::from_be_bytes(b[5..].try_into().expect("infallible")),
        )
    }
}

#[derive(Debug, PartialEq)]
struct Insert {
    timestamp: i32,
    price: i32,
}

impl From<&MessageBytes> for Insert {
    fn from(b: &MessageBytes) -> Self {
        let Message(timestamp, price) = Message::from(b);

        Self { timestamp, price }
    }
}

#[derive(Debug, PartialEq)]
struct Query {
    mintime: i32,
    maxtime: i32,
}

impl From<&MessageBytes> for Query {
    fn from(b: &MessageBytes) -> Self {
        let Message(mintime, maxtime) = Message::from(b);

        Self { mintime, maxtime }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static INSERT_BYTES: MessageBytes = [0x49, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x65];
    static QUERY_BYTES: MessageBytes = [0x51, 0x00, 0x00, 0x03, 0xe8, 0x00, 0x01, 0x86, 0xa0];

    #[test]
    fn make_insert_from_bytes() {
        let msg = Insert::from(&INSERT_BYTES);
        assert_eq!(
            msg,
            Insert {
                timestamp: 12345,
                price: 101
            }
        );
    }

    #[test]
    fn make_query_from_bytes() {
        let msg = Query::from(&QUERY_BYTES);
        assert_eq!(
            msg,
            Query {
                mintime: 1000,
                maxtime: 100000
            }
        );
    }
}

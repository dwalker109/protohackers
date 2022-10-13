use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::task;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let mut conn = Connection::new(listener.accept().await.unwrap().0);

        task::spawn(async move {
            let mut db = Vec::new();

            while let Some(msg) = conn.next_msg().await {
                match msg.0 {
                    MsgType::I => db.push(msg),
                    MsgType::Q => {
                        let matches = db
                            .iter()
                            .filter_map(|i| match (msg.1..=msg.2).contains(&i.1) {
                                true => Some(i64::from(i.2)),
                                false => None,
                            })
                            .collect::<Vec<_>>();

                        let mean = i32::try_from(
                            matches.iter().sum::<i64>() / std::cmp::max(1, matches.len()) as i64,
                        )
                        .expect("mean val fits into an i32");

                        conn.1.write(&mean.to_be_bytes()).await.ok();
                        conn.1.flush().await.ok();
                    }
                }
            }
        });
    }
}

struct Connection(BufReader<OwnedReadHalf>, BufWriter<OwnedWriteHalf>);

impl Connection {
    fn new(stream: TcpStream) -> Self {
        let (r, w) = stream.into_split();
        Self(BufReader::new(r), BufWriter::new(w))
    }

    async fn next_msg(&mut self) -> Option<Msg> {
        let mut buffer = [0x00; 9];
        let n = self.0.read_exact(&mut buffer).await;

        match n {
            Ok(9) => Msg::try_from(&buffer).ok(),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
struct Msg(MsgType, i32, i32);

#[derive(Debug, PartialEq)]
enum MsgType {
    I,
    Q,
}

impl TryFrom<&[u8; 9]> for Msg {
    type Error = &'static str;

    fn try_from(bytes: &[u8; 9]) -> Result<Self, Self::Error> {
        Ok(Self(
            match bytes[0] {
                b'I' => MsgType::I,
                b'Q' => MsgType::Q,
                _ => return Err("invalid msg type"),
            },
            i32::from_be_bytes(bytes[1..5].try_into().unwrap()),
            i32::from_be_bytes(bytes[5..].try_into().unwrap()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static INSERT_BYTES: [u8; 9] = [0x49, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x65];
    static QUERY_BYTES: [u8; 9] = [0x51, 0x00, 0x00, 0x03, 0xe8, 0x00, 0x01, 0x86, 0xa0];
    static BAD_BYTES: [u8; 9] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

    #[test]
    fn make_insert_from_bytes() {
        let msg = Msg::try_from(&INSERT_BYTES).unwrap();
        assert_eq!(msg, Msg(MsgType::I, 12345, 101));
    }

    #[test]
    fn make_query_from_bytes() {
        let msg = Msg::try_from(&QUERY_BYTES).unwrap();
        assert_eq!(msg, Msg(MsgType::Q, 1000, 100000));
    }

    #[test]
    fn bad_msg() {
        let msg = Msg::try_from(&BAD_BYTES);
        assert!(msg.is_err());
    }
}

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let mut db = Vec::new();

    loop {
        let mut conn = Connection::new(listener.accept().await.unwrap().0);
        if let Some(msg) = conn.next_msg().await {
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
            Ok(0) | Err(_) => None,
            Ok(_) => Some(Msg::from(&buffer)),
        }
    }
}

struct Msg(MsgType, i32, i32);

enum MsgType {
    I,
    Q,
}

impl From<&[u8; 9]> for Msg {
    fn from(bytes: &[u8; 9]) -> Self {
        Self(
            match bytes[0] {
                b'I' => MsgType::I,
                b'Q' => MsgType::Q,
                _ => panic!(),
            },
            i32::from_be_bytes(bytes[1..5].try_into().unwrap()),
            i32::from_be_bytes(bytes[5..].try_into().unwrap()),
        )
    }
}

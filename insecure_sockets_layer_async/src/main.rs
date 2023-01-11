use tokio::{net::TcpListener, task};
use tokio_util::codec::FramedRead;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (tcp_stream, _) = listener.accept().await.unwrap();

        task::spawn(async move {
            let mut session = session::Session::new(tcp_stream).await;
        });
    }
}

mod session {
    use bytes::{Buf, BytesMut};
    use futures_util::StreamExt;
    use tokio::net::TcpStream;
    use tokio_util::codec::{Decoder, FramedRead};

    pub enum CipherOp {
        End,
        Rev,
        XorN(u8),
        XorPos,
        AddN(u8),
        AddPos,
    }

    #[derive(Default)]
    pub struct CipherSpec {}

    impl Decoder for CipherSpec {
        type Item = CipherOp;
        type Error = std::io::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            if src.is_empty() {
                return Ok(None);
            }

            let op = match src.get_u8() {
                00 => Self::Item::End,
                01 => Self::Item::Rev,
                02 if !src.is_empty() => Self::Item::XorN(src.get_u8()),
                03 => Self::Item::XorPos,
                04 if !src.is_empty() => Self::Item::AddN(src.get_u8()),
                05 => Self::Item::AddPos,
                02 | 04 => return Ok(None),
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "bad cipherscpec data",
                    ))
                }
            };

            Ok(Some(op))
        }
    }

    pub struct Session {
        tcp_stream: TcpStream,
        cipherspec: Vec<CipherOp>,
        client_pos: usize,
        server_pos: usize,
    }

    impl Session {
        pub async fn new(tcp_stream: TcpStream) -> Self {
            let mut decoder = FramedRead::new(tcp_stream, CipherSpec::default());
            let mut cipherspec = Vec::new();

            while let Some(op) = decoder.next().await {
                match op {
                    Ok(CipherOp::End) => break,
                    Ok(op) => {
                        cipherspec.push(op);
                    }
                    Err(e) => panic!("{e}"),
                }
            }

            Self {
                tcp_stream: decoder.into_inner(),
                cipherspec,
                client_pos: 0,
                server_pos: 0,
            }
        }
    }
}

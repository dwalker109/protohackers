use crate::message::Msg;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use nom::Err::Incomplete;
use tokio::net::TcpStream;
use tokio_util::codec::{Decoder, Encoder, Framed};

#[derive(Debug)]
pub struct MsgCodec;

impl Decoder for MsgCodec {
    type Item = Msg;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (advance_count, result) = match crate::parser::msg(src) {
            Ok((input, msg)) => (src.len() - input.len(), Some(msg)),
            Err(Incomplete(_)) => (0, None),
            Err(e) => (0, Some(Msg::err(&e.to_string()))),
        };

        src.advance(advance_count);

        Ok(result)
    }
}

static ENC_HEAD: usize = 1 + 4;
static ENC_TAIL: usize = 1;

impl Encoder<Msg> for MsgCodec {
    type Error = Box<dyn std::error::Error>;

    fn encode(&mut self, item: Msg, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes: Bytes = match item {
            Msg::Hello { protocol, version } => {
                let protocol = enc_string(&protocol);

                let len = ENC_HEAD + protocol.len() + 4 + ENC_TAIL;

                let mut msg = BytesMut::with_capacity(len);
                msg.put_u8(0x50);
                msg.put_u32(len.try_into()?);
                msg.put(protocol);
                msg.put_u32(version);
                msg.put_u8(enc_checksum(&msg));

                msg.into()
            }
            Msg::Error { message } => {
                let message = enc_string(&message);

                let len = ENC_HEAD + message.len() + ENC_TAIL;

                let mut msg = BytesMut::with_capacity(len);
                msg.put_u8(0x51);
                msg.put_u32(len.try_into()?);
                msg.put(message);
                msg.put_u8(enc_checksum(&msg));

                msg.into()
            }
            Msg::DialAuthority { site } => {
                let len = ENC_HEAD + 4 + ENC_TAIL;

                let mut msg = BytesMut::with_capacity(len);
                msg.put_u8(0x53);
                msg.put_u32(len.try_into()?);
                msg.put_u32(site);
                msg.put_u8(enc_checksum(&msg));

                msg.into()
            }
            Msg::CreatePolicy { species, action } => {
                let species = enc_string(&species);

                let len = ENC_HEAD + species.len() + 1 + ENC_TAIL;

                let mut msg = BytesMut::with_capacity(len);
                msg.put_u8(0x55);
                msg.put_u32(len.try_into()?);
                msg.put(species);
                msg.put_u8(action as u8);
                msg.put_u8(enc_checksum(&msg));

                msg.into()
            }
            Msg::DeletePolicy { policy } => {
                let len = ENC_HEAD + 4 + ENC_TAIL;

                let mut msg = BytesMut::with_capacity(len);
                msg.put_u8(0x56);
                msg.put_u32(len.try_into()?);
                msg.put_u32(policy);
                msg.put_u8(enc_checksum(&msg));

                msg.into()
            }

            Msg::Ok
            | Msg::TargetPopulations { .. }
            | Msg::PolicyResult { .. }
            | Msg::SiteVisit { .. } => {
                unimplemented!()
            }
        };

        dst.extend_from_slice(&bytes);

        Ok(())
    }
}

fn enc_string(s: &str) -> Bytes {
    let mut bytes = BytesMut::with_capacity(4 + s.len());
    bytes.put_u32(s.len().try_into().unwrap());
    bytes.extend(s.as_bytes());

    bytes.into()
}

fn enc_checksum(m: &[u8]) -> u8 {
    (256 - m.iter().copied().map(isize::from).sum::<isize>())
        .rem_euclid(256)
        .try_into()
        .unwrap()
}

#[derive(Debug)]
pub struct MsgFramed(Framed<TcpStream, MsgCodec>);

impl MsgFramed {
    pub async fn new(stream: TcpStream) -> Self {
        let mut msg_framed = Self(Framed::new(stream, MsgCodec));
        msg_framed.preamble().await;

        msg_framed
    }

    pub async fn send(&mut self, msg: Msg) {
        self.0.send(msg).await.ok();
    }

    pub async fn next(&mut self) -> Option<Result<Msg, std::io::Error>> {
        self.0.next().await
    }

    pub async fn preamble(&mut self) {
        self.0
            .send(Msg::Hello {
                protocol: "pestcontrol".to_string(),
                version: 1,
            })
            .await
            .ok();

        if let Some(Ok(msg)) = self.next().await {
            match msg {
                Msg::Hello { .. } => (),
                Msg::Error { .. } => {
                    self.send(msg).await;
                }
                _ => panic!("{msg:?}"),
            }
        }
    }
}

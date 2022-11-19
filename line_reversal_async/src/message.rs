use crate::LcrpBytes;
use std::fmt::Display;

#[derive(Debug, Eq, Hash, PartialEq, Clone, PartialOrd, Ord)]
pub struct Session(Vec<u8>);

impl From<&[u8]> for Session {
    fn from(b: &[u8]) -> Self {
        Self(b.into())
    }
}

impl Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Connect(Session),
    Data(Data),
    Ack(Ack),
    Close(Session),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data {
    pub session: Session,
    pub pos: usize,
    pub data: LcrpBytes,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Ack {
    pub session: Session,
    pub len: usize,
}

impl TryFrom<&[u8]> for Message {
    type Error = Box<dyn std::error::Error>;

    fn try_from(b: &[u8]) -> Result<Self, Self::Error> {
        if b.len() > 1000 {
            return Err("messages must be smaller than 1000 bytes".into());
        }

        let (first, middle) = b.split_first().ok_or("empty message")?;
        let (last, middle) = middle.split_last().ok_or("empty message")?;

        if first != &b'/' || last != &b'/' {
            return Err(
                "contents must begin with a forward slash, end with a forward slash".into(),
            );
        }

        let mut parts = middle.splitn(4, |c| c == &b'/');
        let err_msg = "failed reading next part";

        match parts.next().unwrap() {
            b"connect" => Ok(Self::Connect(Session::from(parts.next().ok_or(err_msg)?))),
            b"data" => {
                let session = Session::from(parts.next().ok_or(err_msg)?);
                let pos = std::str::from_utf8(parts.next().ok_or(err_msg)?)?.parse::<usize>()?;

                let data = String::from_utf8(parts.next().ok_or(err_msg)?.into())?;
                let count_escaped_fwd_slash = data.matches(r"\/").count();
                let data = data.replace(r"\/", "/").replace(r"\\", r"\");
                let count_fwd_slash = data.matches('/').count();

                if count_escaped_fwd_slash != count_fwd_slash {
                    return Err("invalid data, too many parts".into());
                }

                Ok(Self::Data(Data {
                    session,
                    pos,
                    data: data.into_bytes(),
                }))
            }
            b"ack" => Ok(Self::Ack(Ack {
                session: Session::from(parts.next().ok_or(err_msg)?),
                len: std::str::from_utf8(parts.next().ok_or(err_msg)?)?.parse::<usize>()?,
            })),
            b"close" => Ok(Self::Close(Session::from(parts.next().ok_or(err_msg)?))),
            _ => Err("invalid message type".into()),
        }
    }
}

impl From<Message> for String {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Connect(_) => unimplemented!(),
            Message::Data(data) => {
                format!(
                    "/data/{}/{}/{}/",
                    String::from_utf8_lossy(&data.session.0),
                    data.pos,
                    String::from_utf8(data.data.to_vec())
                        .unwrap()
                        .replace('/', r"\/")
                        .replace('\\', r"\\")
                )
            }
            Message::Ack(ack) => {
                format!(
                    "/ack/{}/{}/",
                    String::from_utf8_lossy(&ack.session.0),
                    ack.len
                )
            }
            Message::Close(session) => {
                format!("/close/{}/", String::from_utf8_lossy(&session.0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let m = Message::try_from("/data/123/789/simple message/".as_ref()).unwrap();

        assert_eq!(
            m,
            Message::Data(Data {
                session: Session(b"123".to_vec()),
                pos: 789,
                data: b"simple message".to_vec(),
            })
        )
    }

    #[test]
    fn escapes() {
        let m = Message::try_from("/data/123/789/simple \\/ message/".as_ref()).unwrap();

        println!("{}", String::from(m.clone()));

        assert_eq!(
            m,
            Message::Data(Data {
                session: Session(b"123".to_vec()),
                pos: 789,
                data: b"simple \\/ message".to_vec(),
            })
        )
    }

    #[test]
    fn invalid() {
        let m = Message::try_from("/data/123/789/not/valid/payload/".as_ref());

        assert!(m.is_err());
    }
}

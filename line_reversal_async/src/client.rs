use crate::{
    message::{Data, Session},
    LcrpBytes, MsgQueueCommands,
};
use std::{
    io::{Cursor, Read},
    net::SocketAddr,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct Client {
    pub session: Session,
    pub src: SocketAddr,
    pub in_buf: LcrpBytes,
    pub out_buf: Cursor<LcrpBytes>,
    pub out_acked: usize,
}

impl Client {
    fn sync_buffers(&mut self) {
        let in_buf = &self.in_buf;
        let out_buf = self.out_buf.get_mut();

        while let Some(next_nl) = in_buf.iter().skip(out_buf.len()).position(|c| c == &b'\n') {
            let mut out_line = in_buf[out_buf.len()..out_buf.len() + next_nl]
                .iter()
                .copied()
                .rev()
                .chain([b'\n'])
                .collect::<Vec<_>>();

            out_buf.append(&mut out_line);
        }
    }

    pub async fn send(
        &mut self,
        from: usize,
        src: SocketAddr,
        msg_q_out_sender: UnboundedSender<MsgQueueCommands>,
    ) {
        self.sync_buffers();

        self.out_buf.set_position(u64::try_from(from).unwrap());
        let initial_out_buf_position = self.out_buf.position() as usize;

        let mut data_out = Vec::new();
        self.out_buf.read_to_end(&mut data_out).unwrap();

        let mut pos_offset = 0;
        let msgs = data_out.chunks(768).map(|d| {
            let data = Data {
                session: self.session.clone(),
                pos: initial_out_buf_position + pos_offset,
                data: d.to_vec(),
            };

            pos_offset += d.len();

            data
        });

        for msg in msgs {
            msg_q_out_sender
                .send(MsgQueueCommands::Add(msg.pos + msg.data.len(), src, msg))
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn get_client(init: &str) -> Client {
        Client {
            session: Session::from("1232".to_string().as_bytes()),
            src: SocketAddr::from_str("0.0.0.0:8080").unwrap(),
            in_buf: Vec::from(init.as_bytes()),
            out_buf: Cursor::new(Vec::new()),
            out_acked: 0,
        }
    }

    #[test]
    fn one_msg() {
        let mut c = get_client("Hello\n");

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\n".to_string()
        );
    }

    #[test]
    fn two_msg() {
        let mut c = get_client("Hello\nWorld!\n");

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\n!dlroW\n".to_string()
        );
    }

    #[test]
    fn one_msg_then_two_msg() {
        let mut c = get_client("Hello\n");

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\n".to_string()
        );

        let mut next = b"Another\nWorld!\n".to_vec();
        c.in_buf.append(&mut next);

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\nrehtonA\n!dlroW\n".to_string()
        );
    }

    #[test]
    fn with_incomplete() {
        let mut c = get_client("Hello\nI am not fin...");

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\n".to_string()
        );

        let mut next = b"ished!\n".to_vec();
        c.in_buf.append(&mut next);

        c.sync_buffers();

        assert_eq!(
            String::from_utf8(c.out_buf.get_ref().clone()).unwrap(),
            "olleH\n!dehsi...nif ton ma I\n".to_string()
        );
    }
}

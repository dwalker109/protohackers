use crate::message::Message;
use client::Client;
use message::{Ack, Data, Session};
use std::collections::VecDeque;
use std::time::Duration;
use std::{cmp::Ordering, collections::HashMap, io::Cursor, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::oneshot;
use tokio::time::{interval, sleep};
use tokio::{
    net::UdpSocket,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

type MsgPayload = (SocketAddr, Message);
type LcrpBytes = Vec<u8>;

mod client;
mod message;

#[derive(Debug)]
pub enum MsgQueueCommands {
    Add(usize, SocketAddr, Data),
    Ack(Ack),
    Closed(Session),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:8080").await?);
    eprintln!("started");

    let (udp_sender, udp_receiver) = unbounded_channel();
    tokio::spawn(handle_send_udp(Arc::clone(&socket), udp_receiver));

    let (msg_q_out_sender, msg_q_out_receiver) = unbounded_channel();
    tokio::spawn(handle_msg_out_queue(msg_q_out_receiver, udp_sender.clone()));

    let (msg_in_sender, msg_in_receiver) = channel(512);
    tokio::spawn(handle_msg_in(
        msg_in_receiver,
        udp_sender.clone(),
        msg_q_out_sender,
    ));

    let mut buf = vec![0x00; 1000];
    loop {
        let (amt, src) = socket.recv_from(&mut buf).await?;

        match Message::try_from(&buf[..amt]) {
            Ok(msg) => {
                eprintln!(
                    "received {} from {}",
                    String::from_utf8_lossy(&buf[..amt]),
                    src
                );

                msg_in_sender.send((src, msg)).await.unwrap();
            }
            Err(err) => {
                eprintln!(
                    "coundn't get a message from {} ({err})",
                    String::from_utf8_lossy(&buf[..amt]),
                );
            }
        }
    }
}

async fn handle_send_udp(socket: Arc<UdpSocket>, mut udp_receiver: UnboundedReceiver<MsgPayload>) {
    while let Some((src, msg)) = udp_receiver.recv().await {
        let msg_string: String = msg.into();

        match socket.send_to(msg_string.as_bytes(), src).await {
            Ok(_) => {
                eprintln!("sent {msg_string} to {src}");
            }
            Err(err) => eprintln!("udp send failed, retrying ({err}"),
        }
    }
}

async fn handle_msg_in(
    mut msg_in_receiver: Receiver<MsgPayload>,
    udp_sender: UnboundedSender<MsgPayload>,
    msg_q_out_sender: UnboundedSender<MsgQueueCommands>,
) {
    let mut clients = HashMap::new();

    while let Some((src, msg)) = msg_in_receiver.recv().await {
        match msg {
            Message::Connect(s) => {
                let client = &*clients.entry(s).or_insert_with_key(|s| Client {
                    session: s.to_owned(),
                    src,
                    in_buf: Vec::with_capacity(i32::MAX as usize),
                    out_buf: Cursor::new(Vec::with_capacity(i32::MAX as usize)),
                    out_acked: 0,
                });

                udp_sender
                    .send((
                        client.src,
                        Message::Ack(Ack {
                            session: client.session.clone(),
                            len: client.out_acked,
                        }),
                    ))
                    .unwrap();
            }

            Message::Data(ref data) => {
                if let Some(client) = clients.get_mut(&data.session) {
                    let in_buf_len = client.in_buf.len();

                    match in_buf_len.cmp(&data.pos) {
                        Ordering::Less | Ordering::Greater => {
                            // This message is either before current pos, or later - either way, resend ack stating what we want
                            udp_sender
                                .send((
                                    src,
                                    Message::Ack(Ack {
                                        session: data.session.clone(),
                                        len: in_buf_len,
                                    }),
                                ))
                                .unwrap();
                        }
                        Ordering::Equal => {
                            // We are at the expected pos, store
                            client.in_buf.append(&mut data.data.clone());

                            udp_sender
                                .send((
                                    src,
                                    Message::Ack(Ack {
                                        session: data.session.clone(),
                                        len: client.in_buf.len(),
                                    }),
                                ))
                                .unwrap();

                            client
                                .send(client.out_acked, src, msg_q_out_sender.clone())
                                .await;
                        }
                    }
                } else {
                    udp_sender
                        .send((src, Message::Close(data.session.clone())))
                        .unwrap();
                }
            }

            Message::Ack(ack) => {
                if let Some(client) = clients.get_mut(&ack.session) {
                    if ack.len <= client.out_acked {
                        continue;
                    }

                    if ack.len > client.out_buf.get_ref().len() {
                        udp_sender.send((src, Message::Close(ack.session))).unwrap();
                        continue;
                    }

                    if ack.len < client.out_buf.get_ref().len() {
                        client.send(ack.len, src, msg_q_out_sender.clone()).await;
                        continue;
                    }

                    client.out_acked = ack.len;
                    msg_q_out_sender.send(MsgQueueCommands::Ack(ack)).unwrap();
                } else {
                    udp_sender.send((src, Message::Close(ack.session))).unwrap();
                }
            }

            Message::Close(s) => {
                clients.remove(&s);
                msg_q_out_sender
                    .send(MsgQueueCommands::Closed(s.clone()))
                    .unwrap();
                udp_sender.send((src, Message::Close(s))).unwrap();
            }
        }
    }
}

async fn handle_msg_out_queue(
    mut msg_q_out_receiver: UnboundedReceiver<MsgQueueCommands>,
    udp_sender: UnboundedSender<MsgPayload>,
) {
    let mut queue: HashMap<_, VecDeque<_>> = HashMap::new();

    while let Some(msg) = msg_q_out_receiver.recv().await {
        match msg {
            MsgQueueCommands::Add(expected_ack, src, data) => {
                let (msg_out_sender, msg_out_receiver) = oneshot::channel();
                queue
                    .entry((data.session.clone(), expected_ack))
                    .or_default()
                    .push_back(msg_out_sender);

                tokio::spawn(handle_msg_out(
                    msg_out_receiver,
                    udp_sender.clone(),
                    src,
                    Message::Data(data),
                ));
            }
            MsgQueueCommands::Ack(ack) => {
                if let Some(senders) = queue.get_mut(&(ack.session, ack.len)) {
                    senders.pop_front().into_iter().for_each(|s| {
                        s.send(()).unwrap();
                    });
                }
            }
            MsgQueueCommands::Closed(session) => {
                let to_cancel = queue
                    .keys()
                    .filter(|(s, _)| *s == session)
                    .cloned()
                    .collect::<Vec<_>>();

                for key in to_cancel.iter() {
                    if let Some(senders) = queue.get_mut(key) {
                        senders.pop_front().into_iter().for_each(|s| {
                            s.send(()).unwrap();
                        });
                    }
                }
            }
        }
    }
}

async fn handle_msg_out(
    mut msg_out_receiver: oneshot::Receiver<()>,
    udp_sender: UnboundedSender<MsgPayload>,
    src: SocketAddr,
    msg: Message,
) {
    tokio::pin! {
        let retry = interval(Duration::from_secs(3));
        let timeout = sleep(Duration::from_secs(60));
    }

    loop {
        tokio::select! {
            _ = retry.tick() => {
                udp_sender.send((src, msg.clone())).unwrap();
            }
            _ = &mut msg_out_receiver => {
                break;
            }
            _ = &mut timeout => {
                break
            }
        }
    }
}

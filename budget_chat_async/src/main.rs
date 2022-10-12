use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task;

type Clients = Arc<Mutex<HashMap<String, UnboundedSender<String>>>>;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (tcp_stream, _) = listener.accept().await.unwrap();
        let clients = Arc::clone(&clients);

        task::spawn(async move {
            let (reader, writer) = tcp_stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);
            let (sender, mut receiver) = mpsc::unbounded_channel::<String>();

            writer
                .write_all(b"Welcome to budgetchat async? What shall I call you?\n")
                .await
                .unwrap();
            writer.flush().await.unwrap();

            let name = match get_name(&mut reader, &clients).await {
                Ok(name) => {
                    let current_names = clients
                        .lock()
                        .unwrap()
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    writer
                        .write_all(format!("* The room contains: {current_names}\n").as_bytes())
                        .await
                        .unwrap();
                    writer.flush().await.unwrap();

                    clients.lock().unwrap().insert(name.to_owned(), sender);
                    eprintln!("User {name} connected");

                    name
                }
                Err(reason) => {
                    eprintln!("Could not join, {reason}");
                    eprintln!("Closing connection");

                    return;
                }
            };

            eprintln!("Announcing {name}");
            send_messages(format!("* {name} has entered the room"), &clients, &name);

            let msg_forwarder_thread = task::spawn(async move {
                loop {
                    let msg = receiver.recv().await.unwrap();
                    writer
                        .write_all(format!("{msg}\n").as_bytes())
                        .await
                        .unwrap();
                    writer.flush().await.ok();
                }
            });

            let mut msg = String::new();
            loop {
                if let Ok(n) = reader.read_line(&mut msg).await {
                    eprintln!("{:?}", msg.as_bytes());

                    if n == 0 {
                        eprintln!("Reached EOF for user {name}");
                        break;
                    }

                    send_messages(format!("[{name}] {}", msg.trim_end()), &clients, &name);
                    msg.clear();
                }
            }

            clients.lock().unwrap().remove(&name);
            msg_forwarder_thread.await.ok();

            send_messages(format!("* {name} has left the room"), &clients, &name);

            eprintln!("Disconnected user {name}");
            eprintln!("Closing connection");
        });
    }
}

async fn get_name(
    reader: &mut BufReader<OwnedReadHalf>,
    clients: &Clients,
) -> Result<String, &'static str> {
    let mut name = String::new();
    reader.read_line(&mut name).await.unwrap();
    eprintln!("{:?}", name.as_bytes());
    let name = name.trim();

    if name.is_empty() {
        return Err("no name specified");
    }

    let re = regex::Regex::new(r"^[A-Za-z0-9]*$").unwrap();
    if !re.is_match(name) {
        return Err("invalid name");
    }

    if clients.lock().unwrap().contains_key(name) {
        return Err("duplicate name");
    }

    Ok(name.to_owned())
}

fn send_messages(msg: impl Display, clients: &Clients, sender: &str) {
    for (name, client) in clients.lock().unwrap().iter_mut() {
        match name == sender {
            true => eprintln!("Skipping sending to {sender}"),
            false => {
                client.send(msg.to_string()).ok();
            }
        };
    }
}

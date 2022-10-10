use std::{
    collections::HashMap,
    fmt::Display,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread,
};

type Clients = Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("bound port 80 on 0.0.0.0");
    let clients: Clients = Arc::new(Mutex::new(HashMap::<String, mpsc::Sender<String>>::new()));

    for stream in listener.incoming() {
        let clients = Arc::clone(&clients);

        thread::spawn(move || {
            eprintln!("Opening connection");

            let stream = stream.expect("opened TCP stream");
            let mut reader = BufReader::new(stream.try_clone().expect("cloned stream"));
            let (sender, receiver) = mpsc::channel::<String>();

            writeln!(&stream, "Welcome to budgetchat! What shall I call you?").ok();
            let name = match get_name(&mut reader, &clients) {
                Ok(name) => {
                    let mut clients = clients.lock().unwrap();

                    let current_names = clients.keys().cloned().collect::<Vec<_>>().join(", ");
                    writeln!(&stream, "* The room contains: {current_names}").ok();

                    clients.insert(name.to_owned(), sender);
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

            let msg_forwarder_thread = thread::spawn(move || {
                for msg in receiver {
                    writeln!(&stream, "{msg}").ok();
                }
            });

            let mut msg = String::new();
            while let Ok(n) = reader.read_line(&mut msg) {
                if n == 0 {
                    eprintln!("Reached EOF for user {name}");
                    break;
                }

                send_messages(format!("[{name}] {}", msg.trim_end()), &clients, &name);
                msg.clear();
            }

            clients.lock().unwrap().remove(&name);
            msg_forwarder_thread.join().ok();

            send_messages(format!("* {name} has left the room"), &clients, &name);

            eprintln!("Disconnected user {name}");
            eprintln!("Closing connection");
        });
    }
}

fn get_name(reader: &mut BufReader<TcpStream>, clients: &Clients) -> Result<String, &'static str> {
    let mut name = String::new();
    reader.read_line(&mut name).unwrap();
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

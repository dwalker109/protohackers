use std::{
    collections::HashMap,
    io::{prelude::*, BufReader},
    net::TcpListener,
    sync::{mpsc, Arc, Mutex},
    thread,
};

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("bound port 80 on 0.0.0.0");
    let clients = Arc::new(Mutex::new(HashMap::<String, mpsc::Sender<String>>::new()));

    for stream in listener.incoming() {
        let clients = Arc::clone(&clients);

        thread::spawn(move || {
            eprintln!("Opening connection");

            let stream = stream.expect("opened TCP stream");
            let mut reader = BufReader::new(stream.try_clone().expect("cloned stream"));
            let (sender, receiver) = mpsc::channel::<String>();

            writeln!(&stream, "Welcome to budgetchat! What shall I call you?").ok();

            let mut name = String::new();
            reader.read_line(&mut name).unwrap();
            let name = name.trim();
            clients.lock().unwrap().insert(name.to_owned(), sender);
            eprintln!("User {name} connected");

            let handle = thread::spawn(move || {
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

                for (_name, client) in clients.lock().unwrap().iter_mut() {
                    client.send(msg.clone()).ok();
                }

                msg.clear();
            }

            clients.lock().unwrap().remove(&name.to_owned());
            handle.join().ok();
            eprintln!("Disconnected user {name}");

            eprintln!("Closing connection");
        });
    }
}

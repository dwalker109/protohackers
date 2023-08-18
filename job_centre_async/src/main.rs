use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc, Mutex,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream},
    sync::mpsc,
};

use work::{InFlightQueue, JobQueues};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let next_client_id = AtomicUsize::new(0);
    let job_queues = Arc::new(Mutex::new(work::JobQueues::new()));
    let in_flight_queue = Arc::new(Mutex::new(work::InFlightQueue::new()));

    loop {
        let (tcp_stream, _addr) = listener.accept().await.unwrap();

        tokio::spawn(handle_client(
            tcp_stream,
            next_client_id.fetch_add(1, SeqCst),
            Arc::clone(&job_queues),
            Arc::clone(&in_flight_queue),
        ));
    }
}

async fn handle_client(
    tcp_stream: TcpStream,
    client_id: usize,
    job_queues: Arc<Mutex<JobQueues>>,
    in_flight_queue: Arc<Mutex<InFlightQueue>>,
) {
    let (reader, writer) = tcp_stream.into_split();
    let mut reader = BufReader::new(reader);

    let writer = BufWriter::new(writer);
    let (client_write_tx, client_write_rx) = mpsc::channel::<res::Response>(32);
    tokio::spawn(handle_client_write(writer, client_write_rx));

    let mut buf = String::with_capacity(512);

    loop {
        buf.clear();

        if let Ok(0) = reader.read_line(&mut buf).await {
            eprintln!("EOF");
            break;
        }

        match serde_json::from_str::<req::Request>(buf.trim()) {
            Ok(req::Request::Put { queue, pri, job }) => {
                let id = job_queues.lock().unwrap().add(queue, job, pri);
                client_write_tx
                    .send(res::Response::Put {
                        status: res::ResponseStatus::Ok,
                        id,
                    })
                    .await
                    .ok();
            }
            Ok(req::Request::Get { queues, wait }) => {
                let job = job_queues.lock().unwrap().next_best(&queues);
                match job {
                    Some(job) => {
                        eprintln!("IMMEDIATE {job:?}");
                        in_flight_queue.lock().unwrap().add(job.clone(), client_id);
                        client_write_tx.send(res::Response::from(job)).await.ok();
                    }
                    None if wait == Some(true) => {
                        tokio::spawn(handle_wait_for_job(
                            queues.clone(),
                            Arc::clone(&job_queues),
                            Arc::clone(&in_flight_queue),
                            mpsc::Sender::clone(&client_write_tx),
                            client_id,
                        ));
                    }
                    None => {
                        client_write_tx
                            .send(res::Response::Err {
                                status: res::ResponseStatus::NoJob,
                            })
                            .await
                            .ok();
                    }
                }
            }
            Ok(req::Request::Delete { id }) => {
                let job_queues_deleted = job_queues.lock().unwrap().del(id).is_some();
                let in_flight_deleted = in_flight_queue.lock().unwrap().del(id).is_some();

                let status = if job_queues_deleted || in_flight_deleted {
                    res::ResponseStatus::Ok
                } else {
                    res::ResponseStatus::NoJob
                };

                client_write_tx
                    .send(res::Response::Delete { status })
                    .await
                    .ok();
            }
            Ok(req::Request::Abort { id }) => {
                let res = in_flight_queue.lock().unwrap().abort(
                    &mut job_queues.lock().unwrap(),
                    id,
                    client_id,
                );

                let res = match res {
                    Ok(Some(())) => res::Response::Abort {
                        status: res::ResponseStatus::Ok,
                    },
                    Ok(None) => res::Response::Abort {
                        status: res::ResponseStatus::NoJob,
                    },
                    Err(_) => res::Response::Err {
                        status: res::ResponseStatus::Error,
                    },
                };

                client_write_tx.send(res).await.ok();
            }
            Err(e) => {
                eprint!("Deserialize Err: {e}");
                client_write_tx
                    .send(res::Response::Err {
                        status: res::ResponseStatus::Error,
                    })
                    .await
                    .ok();
            }
        }
    }

    eprintln!("Disconnecting");

    in_flight_queue
        .lock()
        .unwrap()
        .cleanup(&mut job_queues.lock().unwrap(), client_id);
}

async fn handle_client_write(
    mut writer: BufWriter<OwnedWriteHalf>,
    mut client_write_rx: mpsc::Receiver<res::Response>,
) {
    while let Some(val) = client_write_rx.recv().await {
        writer
            .write_all(&serde_json::to_vec(&val).unwrap())
            .await
            .ok();
        writer.write_u8(b'\n').await.ok();
        writer.flush().await.ok();
    }
}

async fn handle_wait_for_job(
    scope_queues: Vec<String>,
    job_queues: Arc<Mutex<JobQueues>>,
    in_flight_queue: Arc<Mutex<InFlightQueue>>,
    client_write_tx: mpsc::Sender<res::Response>,
    client_id: usize,
) {
    let mut subscriber = job_queues.lock().unwrap().subscriber();
    while let Ok(queue) = subscriber.recv().await {
        if scope_queues.contains(&queue) {
            let job = job_queues.lock().unwrap().next_best(&[queue]);
            if let Some(job) = job {
                eprintln!("TASK {job:?}");
                in_flight_queue.lock().unwrap().add(job.clone(), client_id);
                client_write_tx.send(res::Response::from(job)).await.ok();
            }
        }
    }
}

mod req;
mod res;
mod work;

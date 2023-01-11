use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpListener, task};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    while let Ok((tcp_stream, _)) = listener.accept().await {
        task::spawn(async move {
            if let Ok(mut session) = session::Session::new(tcp_stream).await {
                loop {
                    match session.read_stream.next().await {
                        Some(Ok(payload)) => {
                            let pop = payload
                                .split(',')
                                .map(|l| {
                                    let (q, ..) = l.split_once("x ").unwrap_or_default();

                                    (q.parse::<usize>().unwrap(), l)
                                })
                                .max_by_key(|(q, _)| *q)
                                .unwrap()
                                .1
                                .to_owned();

                            session.write_stream.send(pop.clone()).await.unwrap();

                            tracing::info!(pop, "Sent result");
                        }
                        Some(Err(err)) => {
                            tracing::error!(err = ?err);
                            break;
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        });
    }
}

mod session;

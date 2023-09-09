use crate::message::VisitPopulation;
use codec::MsgFramed;
use message::Msg;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let (sv_send, sv_recv) = mpsc::channel::<(u32, Vec<VisitPopulation>)>(1);

    task::spawn(handle_site_visit(sv_recv));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let sv_send = sv_send.clone();

        task::spawn(async move {
            let mut msg_framed = MsgFramed::new(stream).await;

            while let Some(Ok(msg)) = msg_framed.next().await {
                match msg {
                    Msg::SiteVisit { site, populations } => {
                        sv_send.send((site, populations)).await.unwrap()
                    }
                    Msg::Error { .. } => msg_framed.send(msg).await,
                    _ => panic! {"{msg:?}"},
                }
            }
        });
    }
}

async fn handle_site_visit(mut recv: mpsc::Receiver<(u32, Vec<VisitPopulation>)>) {
    let mut state = state::State::new();

    while let Some((site_id, visit_populations)) = recv.recv().await {
        eprintln!("Starting site {site_id}");
        state.init_site(&site_id).await;
        state.process_site(&site_id, &visit_populations).await;
        eprintln!("Ended site {site_id}");
    }
}

mod codec;
mod message;
mod parser;
mod state;

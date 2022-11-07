use crate::connection::{frame_rw, Frame, FrameReader};
use crate::message::{IAmCamera, IAmDispatcher, InMessage, OutMessage, Plate, Ticket};
use connection::FrameWriter;
use message::Error;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task;
use tokio::time::{interval, MissedTickBehavior};

type CameraDb = Arc<Mutex<HashMap<IAmCamera, Vec<Plate>>>>;
type DispatcherDb = Arc<Mutex<Vec<(IAmDispatcher, UnboundedSender<OutMessage>)>>>;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let camera_db: CameraDb = Arc::new(Mutex::new(HashMap::new()));
    let dispatcher_db: DispatcherDb = Arc::new(Mutex::new(Vec::new()));

    tokio::spawn(handle_enforcement(
        Arc::clone(&camera_db),
        Arc::clone(&dispatcher_db),
    ));

    loop {
        let (frame_reader, frame_writer) = frame_rw(listener.accept().await.unwrap().0);
        let (out_message_sender, out_message_receiver) = unbounded_channel::<OutMessage>();

        tokio::spawn(handle_messages_to_frames(
            out_message_receiver,
            frame_writer,
        ));

        task::spawn(handle_frames_in(
            frame_reader,
            out_message_sender,
            Arc::clone(&camera_db),
            Arc::clone(&dispatcher_db),
        ));
    }
}

async fn handle_messages_to_frames(
    mut out_message_receiver: UnboundedReceiver<OutMessage>,
    mut frame_writer: FrameWriter,
) {
    loop {
        if let Some(msg) = out_message_receiver.recv().await {
            frame_writer.write(Frame::from(msg)).await.ok();
        }
    }
}

async fn handle_heartbeat(sender: UnboundedSender<OutMessage>, interval: u32) {
    if interval == 0 {
        return;
    };

    let mut interval = tokio::time::interval(Duration::from_millis(u64::from(interval) * 100));

    while !sender.is_closed() {
        sender.send(OutMessage::Heartbeat).ok();
        interval.tick().await;
    }
}

async fn handle_frames_in(
    mut frame_reader: FrameReader,
    out_message_sender: UnboundedSender<OutMessage>,
    camera_db: CameraDb,
    dispatcher_db: DispatcherDb,
) {
    let mut client_identified = false;

    while let Ok(init_frame) = frame_reader.read().await {
        if let Some(init_frame) = init_frame {
            match InMessage::from(init_frame) {
                InMessage::WantHeartbeat(msg) => {
                    tokio::spawn(handle_heartbeat(out_message_sender.clone(), msg.interval));
                }

                InMessage::IAmCamera(_) | InMessage::IAmDispatcher(_) if client_identified => {
                    out_message_sender
                        .send(OutMessage::Error(Error::from(
                            "client tried to identify twice".to_string(),
                        )))
                        .ok();
                }

                InMessage::IAmCamera(camera) => {
                    camera_db.lock().unwrap().insert(camera, Vec::new());
                    client_identified = true;

                    while let Ok(next_frame) = frame_reader.read().await {
                        if let Some(next_frame) = next_frame {
                            match InMessage::from(next_frame) {
                                InMessage::Plate(plate) => {
                                    camera_db
                                        .lock()
                                        .unwrap()
                                        .entry(camera)
                                        .and_modify(|plates| plates.push(plate));
                                }
                                InMessage::WantHeartbeat(msg) => {
                                    tokio::spawn(handle_heartbeat(
                                        out_message_sender.clone(),
                                        msg.interval,
                                    ));
                                }
                                _ => {
                                    out_message_sender
                                        .send(OutMessage::Error(Error::from(
                                            "bad camera plate request".to_string(),
                                        )))
                                        .ok();
                                }
                            }
                        };
                    }
                }

                InMessage::IAmDispatcher(dispatcher) => {
                    client_identified = true;
                    dispatcher_db
                        .lock()
                        .unwrap()
                        .push((dispatcher, out_message_sender.clone()));
                }

                InMessage::Error(err) => {
                    out_message_sender.send(OutMessage::Error(err)).ok();
                }

                _ => {
                    out_message_sender
                        .send(OutMessage::Error(Error::from(
                            "bad client connect frame request".to_string(),
                        )))
                        .ok();
                }
            }
        }
    }
}

async fn handle_enforcement(camera_db: CameraDb, dispatcher_db: DispatcherDb) {
    let mut history = HashMap::new();

    let mut limiter = interval(Duration::from_secs(5));
    limiter.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        limiter.tick().await;

        let mut tickets = VecDeque::new();
        let mut by_road = HashMap::new();

        for (camera, plates) in camera_db.lock().unwrap().iter() {
            for plate in plates {
                let entry = by_road
                    .entry((camera.road, camera.limit))
                    .or_insert_with(Vec::new);
                entry.push((camera.mile, plate.timestamp, plate.plate.clone()));
            }
        }

        for ((road, limit), observations) in by_road.iter() {
            for (mile_a, timestamp_a, plate_a) in observations {
                let limit = f64::from(*limit) + 0.5;
                let new_tickets = observations
                    .iter()
                    .filter(|(_, _, plate_b)| plate_a == plate_b)
                    .filter(|(mile_b, timestamp_b, _)| {
                        (mile_a, timestamp_a) != (mile_b, timestamp_b)
                    })
                    .filter_map(|(mile_b, timestamp_b, _)| {
                        let distance = f64::from(mile_a.abs_diff(*mile_b));
                        let time = f64::from(timestamp_a.abs_diff(*timestamp_b)) / 60_f64 / 60_f64;
                        let speed = distance / time;

                        let (mile1, timestamp1, mile2, timestamp2) = match timestamp_a
                            .cmp(timestamp_b)
                        {
                            std::cmp::Ordering::Less => (mile_a, timestamp_a, mile_b, timestamp_b),
                            std::cmp::Ordering::Greater => {
                                (mile_b, timestamp_b, mile_a, timestamp_a)
                            }
                            std::cmp::Ordering::Equal => unimplemented!(),
                        };

                        match speed > limit {
                            true => Some(Ticket {
                                plate: plate_a.clone(),
                                road: *road,
                                mile1: *mile1,
                                timestamp1: *timestamp1,
                                mile2: *mile2,
                                timestamp2: *timestamp2,
                                speed: (speed * 100_f64) as u16,
                            }),
                            false => None,
                        }
                    });

                tickets.extend(new_tickets);
            }
        }

        while let Some(t) = tickets.pop_front() {
            let day1 = t.timestamp1 / 86400;
            let day2 = t.timestamp2 / 86400;

            let days = history.entry(t.plate.clone()).or_insert_with(Vec::new);

            if days.contains(&day1) || days.contains(&day2) {
                continue;
            }

            if let Some((_, sender)) = dispatcher_db
                .lock()
                .unwrap()
                .iter()
                .find(|(msg, _)| msg.roads.contains(&t.road))
            {
                days.push(day1);
                days.push(day2);

                sender.send(OutMessage::Ticket(t)).ok();
            }
        }
    }
}

mod connection;
mod message;

use crate::connection::Frame;

pub enum InMessage {
    Error(Error),
    Plate(Plate),
    WantHeartbeat(WantHeartbeat),
    IAmCamera(IAmCamera),
    IAmDispatcher(IAmDispatcher),
}

pub enum OutMessage {
    Error(Error),
    Ticket(Ticket),
    Heartbeat,
}

pub struct Error {
    msg: String,
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self { msg }
    }
}

pub struct Plate {
    pub plate: String,
    pub timestamp: u32,
}

pub struct Ticket {
    pub plate: String,
    pub road: u16,
    pub mile1: u16,
    pub timestamp1: u32,
    pub mile2: u16,
    pub timestamp2: u32,
    pub speed: u16,
}

pub struct WantHeartbeat {
    pub interval: u32,
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct IAmCamera {
    pub road: u16,
    pub mile: u16,
    pub limit: u16,
}

pub struct IAmDispatcher {
    num_roads: u8,
    pub roads: Vec<u16>,
}

impl From<Frame> for InMessage {
    fn from(frame: Frame) -> Self {
        match frame {
            Frame::Error(msg) => InMessage::Error(Error::from(msg)),
            Frame::Plate(plate, timestamp) => InMessage::Plate(Plate { plate, timestamp }),
            Frame::WantHeartbeat(interval) => InMessage::WantHeartbeat(WantHeartbeat { interval }),
            Frame::IAmCamera(road, mile, limit) => {
                InMessage::IAmCamera(IAmCamera { road, mile, limit })
            }
            Frame::IAmDispatcher(num_roads, roads) => {
                InMessage::IAmDispatcher(IAmDispatcher { num_roads, roads })
            }
            _ => unimplemented!(),
        }
    }
}

impl From<OutMessage> for Frame {
    fn from(message: OutMessage) -> Self {
        match message {
            OutMessage::Error(Error { msg }) => Frame::Error(msg),
            OutMessage::Ticket(Ticket {
                plate,
                road,
                mile1,
                timestamp1,
                mile2,
                timestamp2,
                speed,
            }) => Frame::Ticket(plate, road, mile1, timestamp1, mile2, timestamp2, speed),
            OutMessage::Heartbeat => Frame::Heartbeat,
        }
    }
}

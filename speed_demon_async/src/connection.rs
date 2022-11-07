use bytes::{Buf, BytesMut};
use std::fmt::{Display, Formatter};
use std::io::Cursor;
use std::string::FromUtf8Error;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::time::{interval, Interval, MissedTickBehavior};

pub enum Frame {
    Error(String),
    Plate(String, u32),
    Ticket(String, u16, u16, u32, u16, u32, u16),
    WantHeartbeat(u32),
    Heartbeat,
    IAmCamera(u16, u16, u16),
    IAmDispatcher(u8, Vec<u16>),
}

pub struct FrameReader {
    tcp_stream_r: OwnedReadHalf,
    stream_buf: BytesMut,
    limiter: Interval,
}

pub struct FrameWriter {
    tcp_stream_w: OwnedWriteHalf,
}

pub fn frame_rw(tcp_stream: TcpStream) -> (FrameReader, FrameWriter) {
    let (r, w) = tcp_stream.into_split();

    (FrameReader::new(r), FrameWriter::new(w))
}

impl FrameReader {
    pub fn new(tcp_stream_r: OwnedReadHalf) -> Self {
        let mut limiter = interval(Duration::from_micros(1_000));
        limiter.set_missed_tick_behavior(MissedTickBehavior::Skip);

        Self {
            tcp_stream_r,
            stream_buf: BytesMut::new(),
            limiter,
        }
    }

    pub async fn read(&mut self) -> Result<Option<Frame>, FrameError> {
        loop {
            match self.try_next() {
                Ok(frame) => return Ok(Some(frame)),
                Err(err) if matches!(err, FrameError::Incomplete) => {
                    let bytes_read = self.tcp_stream_r.read_buf(&mut self.stream_buf).await?;

                    if bytes_read == 0 {
                        self.limiter.tick().await;

                        return match self.stream_buf.is_empty() {
                            true => Ok(None),
                            false => Err("peer disconnected prematurely".into()),
                        };
                    }
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn try_next(&mut self) -> Result<Frame, FrameError> {
        let mut c = Cursor::new(&self.stream_buf[..]);

        let type_byte = next_u8(&mut c)?;

        let frame = match type_byte {
            0x20 => {
                let plate = next_string(&mut c)?;
                let timestamp = next_u32(&mut c)?;

                Frame::Plate(plate, timestamp)
            }
            0x40 => {
                let interval = next_u32(&mut c)?;

                Frame::WantHeartbeat(interval)
            }
            0x80 => {
                let road = next_u16(&mut c)?;
                let mile = next_u16(&mut c)?;
                let limit = next_u16(&mut c)?;

                Frame::IAmCamera(road, mile, limit)
            }
            0x81 => {
                let num_roads = next_u8(&mut c)?;
                let roads = next_u16_vec(&mut c, &usize::from(num_roads))?;

                Frame::IAmDispatcher(num_roads, roads)
            }
            _ => Frame::Error(format!("unsupported message type received ({type_byte})")),
        };

        self.stream_buf
            .advance(usize::try_from(c.position()).unwrap());

        Ok(frame)
    }
}

impl FrameWriter {
    pub fn new(tcp_stream_w: OwnedWriteHalf) -> Self {
        Self { tcp_stream_w }
    }

    pub async fn write(&mut self, frame: Frame) -> tokio::io::Result<()> {
        match frame {
            Frame::Error(msg) => {
                self.tcp_stream_w.write_u8(0x10).await?;
                self.tcp_stream_w
                    .write_u8(u8::try_from(msg.len()).unwrap())
                    .await?;
                self.tcp_stream_w.write_all(msg.as_bytes()).await?;
            }
            Frame::Ticket(plate, road, mile1, timestamp1, mile2, timestamp2, speed) => {
                self.tcp_stream_w.write_u8(0x21).await?;
                self.tcp_stream_w
                    .write_u8(u8::try_from(plate.len()).unwrap())
                    .await?;
                self.tcp_stream_w.write_all(plate.as_bytes()).await?;
                self.tcp_stream_w.write_u16(road).await?;
                self.tcp_stream_w.write_u16(mile1).await?;
                self.tcp_stream_w.write_u32(timestamp1).await?;
                self.tcp_stream_w.write_u16(mile2).await?;
                self.tcp_stream_w.write_u32(timestamp2).await?;
                self.tcp_stream_w.write_u16(speed).await?;
            }
            Frame::Heartbeat => {
                self.tcp_stream_w.write_u8(0x41).await?;
            }
            _ => unimplemented!(),
        }

        self.tcp_stream_w.flush().await?;

        Ok(())
    }
}

fn next_u8(b: &mut Cursor<&[u8]>) -> Result<u8, FrameError> {
    b.has_remaining()
        .then(|| b.get_u8())
        .ok_or(FrameError::Incomplete)
}

fn next_u16(b: &mut Cursor<&[u8]>) -> Result<u16, FrameError> {
    (b.remaining() >= 2)
        .then(|| b.get_u16())
        .ok_or(FrameError::Incomplete)
}

fn next_u32(b: &mut Cursor<&[u8]>) -> Result<u32, FrameError> {
    (b.remaining() >= 4)
        .then(|| b.get_u32())
        .ok_or(FrameError::Incomplete)
}

fn next_string(b: &mut Cursor<&[u8]>) -> Result<String, FrameError> {
    let len = usize::from(next_u8(b)?);
    let dat = (b.remaining() >= len)
        .then(|| b.copy_to_bytes(len))
        .ok_or(FrameError::Incomplete)?;

    Ok(String::from_utf8(dat.to_vec())?)
}

fn next_u16_vec(b: &mut Cursor<&[u8]>, len: &usize) -> Result<Vec<u16>, FrameError> {
    let mut dat = Vec::with_capacity(*len);
    for _ in 0..*len {
        dat.push(next_u16(b)?);
    }

    Ok(dat)
}

#[derive(Debug)]
pub enum FrameError {
    Incomplete,
    Fatal(Box<dyn std::error::Error + Send + Sync>),
}

impl std::error::Error for FrameError {}

impl Display for FrameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete => write!(f, "incomplete frame"),
            Self::Fatal(m) => write!(f, "{m}"),
        }
    }
}

impl From<&str> for FrameError {
    fn from(msg: &str) -> Self {
        Self::Fatal(msg.into())
    }
}

impl From<FromUtf8Error> for FrameError {
    fn from(e: FromUtf8Error) -> Self {
        Self::Fatal(e.into())
    }
}

impl From<tokio::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        Self::Fatal(e.into())
    }
}

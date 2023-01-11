use bytes::{Buf, BufMut, BytesMut};
use futures_util::StreamExt;
use std::result::Result;
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};
use tracing;

#[derive(Clone, Debug)]
pub enum CipherOp {
    End,
    Rev,
    XorN(u8),
    XorPos,
    AddN(u8),
    AddPos,
}

impl CipherOp {
    fn proc_byte(&self, b: u8, pos: usize) -> u8 {
        match self {
            CipherOp::End => unimplemented!(),
            CipherOp::Rev => b.reverse_bits(),
            CipherOp::XorN(n) => b ^ n,
            CipherOp::XorPos => b ^ pos as u8,
            CipherOp::AddN(n) => b.wrapping_add(*n),
            CipherOp::AddPos => b.wrapping_add(pos as u8),
        }
    }

    fn proc_byte_inverse(&self, b: u8, pos: usize) -> u8 {
        match self {
            CipherOp::End | CipherOp::Rev | CipherOp::XorN(..) | CipherOp::XorPos => {
                self.proc_byte(b, pos)
            }
            CipherOp::AddN(n) => b.wrapping_sub(*n),
            CipherOp::AddPos => b.wrapping_sub(pos as u8),
        }
    }
}

#[derive(Default, Debug)]
pub struct CipherSpec {}

impl Decoder for CipherSpec {
    type Item = CipherOp;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        let op = match src.get_u8() {
            00 => Self::Item::End,
            01 => Self::Item::Rev,
            02 if !src.is_empty() => Self::Item::XorN(src.get_u8()),
            03 => Self::Item::XorPos,
            04 if !src.is_empty() => Self::Item::AddN(src.get_u8()),
            05 => Self::Item::AddPos,
            02 | 04 => return Ok(None),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bad cipherscpec data",
                ))
            }
        };

        Ok(Some(op))
    }
}

#[derive(Default)]
pub struct ToysList {
    pos: usize,
    cipherspec: Vec<CipherOp>,
}

impl Decoder for ToysList {
    type Item = String;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut working_src = src.clone();
        let mut text = BytesMut::new();
        let mut n = 0;

        loop {
            n += 1;

            if working_src.is_empty() {
                // Allow the src buffer to fill and come back later
                return Ok(None);
            }

            // Process a byte
            let mut b = working_src.get_u8();
            for op in self.cipherspec.iter() {
                b = op.proc_byte_inverse(b, self.pos + text.len());
            }

            if b == b'\n' {
                // Found a line end - advance the src buffer and current pos
                src.advance(n);
                self.pos += n;

                return match String::from_utf8(text.to_vec()) {
                    Ok(t) => Ok(Some(t)),
                    Err(e) => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("bad decoded data: {e}"),
                    )),
                };
            } else {
                // Not a line end, accumulate
                text.put_u8(b);
            }
        }
    }
}

impl Encoder<String> for ToysList {
    type Error = std::io::Error;

    fn encode(&mut self, mut item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.push('\n');

        for mut b in item.bytes() {
            for op in self.cipherspec.iter() {
                b = op.proc_byte(b, self.pos + dst.len());
            }
            dst.put_u8(b);
        }

        self.pos += dst.len();

        Ok(())
    }
}

pub struct Session {
    pub read_stream: FramedRead<OwnedReadHalf, ToysList>,
    pub write_stream: FramedWrite<OwnedWriteHalf, ToysList>,
}

impl Session {
    #[tracing::instrument]
    pub async fn new(tcp_stream: TcpStream) -> Result<Self, ()> {
        let (r, w) = tcp_stream.into_split();

        let mut decoder = FramedRead::new(r, CipherSpec::default());
        let mut cipherspec = Vec::new();

        while let Some(op) = decoder.next().await {
            match op {
                Ok(CipherOp::End) => break,
                Ok(op) => {
                    cipherspec.push(op);
                }
                Err(e) => {
                    tracing::debug!("error building cipherspec: {e}");
                    return Err(());
                }
            }
        }

        // Sanity check
        let control = b"abc123";
        let mut test = *control;
        for (n, c) in control.iter().enumerate() {
            let mut t = *c;
            for op in cipherspec.iter() {
                t = op.proc_byte(t, n);
            }
            test[n] = t;
        }
        if control == &test {
            tracing::debug!(cipherspec = ?cipherspec);
            return Err(());
        }

        Ok(Self {
            // Re-use the TCP half (and potentially non empty underlying buffer)
            // while switching to the ToysList decoder impl
            read_stream: decoder.map_decoder(|_| ToysList {
                pos: 0,
                cipherspec: cipherspec.clone().into_iter().rev().collect(),
            }),
            // New ToysList encoder with write half split out earlier
            write_stream: FramedWrite::new(w, ToysList { pos: 0, cipherspec }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ex_1() {
        let input: [u8; 5] = [0x68, 0x65, 0x6c, 0x6c, 0x6f];
        let cipherspec = vec![CipherOp::XorN(1), CipherOp::Rev];

        let mut res = Vec::with_capacity(5);

        for x in input {
            let mut y = x;
            for op in cipherspec.iter() {
                y = op.proc_byte(y, 0);
            }
            res.push(y);
        }

        assert_eq!(&res, &[0x96, 0x26, 0xb6, 0xb6, 0x76]);
    }

    #[test]
    fn ex_2() {
        let input: [u8; 5] = [0x68, 0x65, 0x6c, 0x6c, 0x6f];
        let cipherspec = vec![CipherOp::AddPos, CipherOp::AddPos];

        let mut res = Vec::with_capacity(5);

        for (pos, x) in input.iter().enumerate() {
            let mut y = *x;
            for op in cipherspec.iter() {
                y = op.proc_byte(y, pos);
            }
            res.push(y);
        }

        assert_eq!(&res, &[0x68, 0x67, 0x70, 0x72, 0x77]);
    }

    #[test]
    fn ex_3() {
        let input_1 = [
            0xf2, 0x20, 0xba, 0x44, 0x18, 0x84, 0xba, 0xaa, 0xd0, 0x26, 0x44, 0xa4, 0xa8, 0x7e,
        ];
        let input_2 = [
            0x6a, 0x48, 0xd6, 0x58, 0x34, 0x44, 0xd6, 0x7a, 0x98, 0x4e, 0x0c, 0xcc, 0x94, 0x31,
        ];
        let output_1 = [0x72, 0x20, 0xba, 0xd8, 0x78, 0x70, 0xee];
        let output_2 = [0xf2, 0xd0, 0x26, 0xc8, 0xa4, 0xd8, 0x7e];

        let cipherspec = vec![CipherOp::XorN(123), CipherOp::AddPos, CipherOp::Rev];

        let res = |input: &[u8], decoding: bool, offset: usize| {
            let mut res = Vec::with_capacity(14);
            for (pos, x) in input.iter().enumerate() {
                let mut y = *x;
                if decoding {
                    for op in cipherspec.iter().rev() {
                        y = op.proc_byte_inverse(y, pos + offset);
                    }
                } else {
                    for op in cipherspec.iter() {
                        y = op.proc_byte(y, pos + offset);
                    }
                }
                res.push(y);
            }

            res
        };

        assert_eq!(
            res(&input_1, true, 0),
            "4x dog,5x car\n".to_string().as_bytes()
        );

        assert_eq!(res(b"5x car\n", false, 0), output_1);

        assert_eq!(
            res(&input_2, true, input_1.len()),
            "3x rat,2x cat\n".to_string().as_bytes()
        );

        assert_eq!(res(b"3x rat\n", false, output_1.len()), output_2);
    }
}

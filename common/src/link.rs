use serde_cbor::error::Error as CborError;
use cobs::{max_encoding_length, encode, decode};
use crate::message::Message;
use static_assertions::const_assert;

#[derive(Debug)]
pub enum LinkError {
    Cbor(CborError),
    PacketTooLarge,
    PacketNoStop,
    Cobs,
    DestTooSmall,
}

impl From<CborError> for LinkError {
    fn from(e: CborError) -> LinkError {
        LinkError::Cbor(e)
    }
}

type Result<T> = core::result::Result<T, LinkError>;

const MAX_PACKET_SIZE: usize = Message::MAX_SIZE;
const_assert!(MAX_PACKET_SIZE < u16::MAX as usize);
struct Packet {
    size: u16,
    write: u16,
    // read: u16,
    data: [u8; MAX_PACKET_SIZE],
}

impl Packet {
    const START: u8 = 0x01;
    const STOP: u8 = 0x00;
    const HEADER_SIZE: usize = 3;
    fn new() -> Packet {
        Packet {
            size: 0,
            write: 0,
            // read: 0,
            data: [0u8; MAX_PACKET_SIZE],
        }
    }

    fn new_with_size(size: u16) -> Result<Self> {
        let mut packet = Self::new();
        packet.set_header(size)?;
        packet.set_write(Self::HEADER_SIZE as u16);
        Ok(packet)
    }

    fn set_header(&mut self, size: u16) -> Result<()> {
        if size as usize > MAX_PACKET_SIZE - Self::HEADER_SIZE {
            return Err(LinkError::PacketTooLarge);
        }
        self.size = size;
        self.buffer_write_start();
        self.buffer_write_size(size);
        Ok(())
    }

    fn set_write(&mut self, write: u16) {
        self.write = write;
    }

    fn buffer_write_stop(&mut self) {
        self.data[self.write as usize] = Self::STOP;
        self.write += 1;
        self.size += 1;
    }

    fn buffer_write_start(&mut self) {
        self.data[0] = Self::START;
    }

    fn buffer_write_size(&mut self, size: u16) {
        let size = size.to_le_bytes();
        self.data[1] = size[0];
        self.data[2] = size[1];
    }

    // pub fn get_size(&self) -> usize {
    //     self.size as usize
    // }

    fn push(&mut self, data: u8) -> bool {
        self.data[self.write as usize] = data;
        self.write += 1;
        self.write == self.size
    }

    fn decode_data(&self, output: &mut [u8]) -> Result<usize> {
        if output.len() < self.size as usize - Self::HEADER_SIZE {
            return Err(LinkError::DestTooSmall);
        }
        match decode(&self.data[Self::HEADER_SIZE..self.size as usize], output) {
            Ok(size) => Ok(size),
            Err(_) => Err(LinkError::Cobs),
        }
    }

    fn build(data: &[u8]) -> Result<Packet> {
        if data.len() > MAX_PACKET_SIZE || max_encoding_length(data.len()) > MAX_PACKET_SIZE {
            return Err(LinkError::PacketTooLarge);
        }
        let mut packet = Packet::new();
        let size = encode(data, &mut packet.data[Self::HEADER_SIZE..]) + Self::HEADER_SIZE;
        packet.set_header(size as u16)?;
        packet.set_write(size as u16);
        packet.buffer_write_stop();
        Ok(packet)
    }

    fn to_bytes(self) -> (usize, [u8; MAX_PACKET_SIZE]) {
        (self.size as usize, self.data)
    }
}

enum RxState {
    Idle,
    Start,
    Size(u8),
    Data(Packet),
    Stop(Packet),
}

struct RxStateMachine { inner: Option<RxState> }

impl RxStateMachine {
    fn new() -> RxStateMachine {
        RxStateMachine { inner: Some(RxState::Idle) }
    }

    fn turn(&mut self, data: u8) -> Result<Option<Packet>> {
        use RxState::*;
        let mut res = Ok(None);
        let next_state = match self.inner.take().unwrap() {
            Idle => {
                if Packet::START == data {
                    Start
                } else {
                    Idle
                }
            }
            Start => {
                Size(data)
            }
            Size(lb) => {
                let size = u16::from_le_bytes([lb, data]);
                match Packet::new_with_size(size) {
                    Ok(packet) => Data(packet),
                    Err(e) => {
                        res = Err(e);
                        Idle
                    }
                }
            }
            Data(mut packet) => {
                if packet.push(data) {
                    Stop(packet)
                } else {
                    Data(packet)
                }
            }
            Stop(packet) => {
                if Packet::STOP == data {
                    res = Ok(Some(packet));
                    Idle
                } else {
                    res = Err(LinkError::PacketNoStop);
                    Idle
                }
            }
        };
        self.inner = Some(next_state);
        return res;
    }
}

pub struct Link {
    rx_state: RxStateMachine,
}

impl Link {
    pub fn new() -> Self {
        Self {
            rx_state : RxStateMachine::new(),
        }
    }

    pub fn encode(&mut self, message: &Message) -> Result<(usize, [u8; Message::MAX_SIZE])> {
        let mut buf = [0u8; Message::MAX_SIZE];
        let size = message.write_bytes(&mut buf)?;
        Ok(Packet::build(&buf[..size])?.to_bytes())
    }

    pub fn push(&mut self, data: u8) -> Result<Option<Message>> {
        if let Some(packet) = self.rx_state.turn(data)? {
            let mut buf = [0u8; Message::MAX_SIZE];
            let size = packet.decode_data(&mut buf[..])?;
            Ok(Some(Message::from_bytes(&buf[..size])?))
        } else {
            Ok(None)
        }
    }

    pub fn push_slice(&mut self, data: &[u8]) -> Result<(usize, Option<Message>)> {
        let mut i = 0;
        for b in data {
            let res = self.push(*b)?;
            i += 1;
            if res.is_some() {
                return Ok((i, res));
            }
        }
        Ok((i, None))
    }
}

#[cfg(test)]
mod test {
    use crate::Message;
    use super::Link;

    fn echo_test(msg: Message) {
        let mut link = Link::new();
        let (size, buf) = link.encode(&msg).unwrap();
        let (used, rx) = link.push_slice(&buf[..size]).unwrap();
        assert_eq!(used, size);
        assert_eq!(msg, rx.unwrap());
    }

    #[test]
    fn say_hello() {
        let msg = Message::Hello;
        let mut link = Link::new();
        let (size, buf) = link.encode(&msg).unwrap();
        for b in &buf[..size] {
            let res: Option<Message> = link.push(*b).unwrap();
            if let Some(rx) = res {
                assert_eq!(msg, rx);
                break;
            }
        }
    }

    // #[test]
    // fn cobs_test() {
    //     let msg = Message::Hello;
    //     let mut buf = [0u8; 100];
    //     let size = msg.write_bytes(&mut buf).unwrap();
    //     println!("Size: {} buf: {:?}", size, &buf[..size]);
    //     let mut output_enc = [0u8; 100];

    //     let outsize = encode(&buf[..size], &mut output_enc);
    //     println!("Size: {} buf: {:?}", outsize, &output_enc[..outsize]);
    //     let mut output_dec = [0u8; 100];

    //     let outsize = decode(&output_enc[..outsize], &mut output_dec).unwrap();
    //     println!("Size: {} buf: {:?}", outsize, &output_dec[..outsize]);


    // }

    #[test]
    fn say_hello_slice() {
        echo_test(Message::Hello);
    }

    #[test]
    fn large_buffer() {
        echo_test(Message::log(b"Hello, World"))
    }
}

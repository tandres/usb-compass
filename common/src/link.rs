use serde_cbor::error::Error as CborError;
use serial_line_ip::{Encoder, Decoder, Error as SlipError};
use crate::message::Message;
use static_assertions::const_assert;

#[derive(Debug)]
pub enum LinkError {
    Cbor(CborError),
    Slip(SlipError),
}

impl From<CborError> for LinkError {
    fn from(e: CborError) -> LinkError {
        LinkError::Cbor(e)
    }
}

impl From<SlipError> for LinkError {
    fn from(e: SlipError) -> LinkError {
        LinkError::Slip(e)
    }
}

type Result<T> = core::result::Result<T, LinkError>;

const MAX_PACKET_SIZE: usize = Message::MAX_SIZE;
const_assert!(MAX_PACKET_SIZE < u16::MAX as usize);

pub struct Link {
    decoder: Decoder,
    scratch_offset: usize,
    scratch: [u8; MAX_PACKET_SIZE],
}

impl Link {
    pub fn new() -> Link {
        Link {
            decoder: Decoder::new(),
            scratch_offset: 0,
            scratch: [0u8; MAX_PACKET_SIZE],
        }
    }

    pub fn encode(&mut self, msg: &Message, output: &mut [u8]) -> Result<usize> {
        let mut encoder = Encoder::new();
        let mut buf = [0u8; MAX_PACKET_SIZE];
        let size = msg.write_bytes(&mut buf)?;
        let mut totals = encoder.encode(&buf[..size], output)?;
        totals += encoder.finish(&mut output[totals.written..])?;
        Ok(totals.written)
    }

    pub fn decode(&mut self, buf: &[u8]) -> Result<(usize, Option<Message>)> {
        let (read, packet, present) = self.decoder.decode(buf, &mut self.scratch[self.scratch_offset..])?;
        self.scratch_offset += packet.len();
        let mut res = None;
        if present {
            if packet.len() != 0 {
                match Message::from_bytes(&self.scratch[..self.scratch_offset]) {
                    Ok(msg) => {
                        res = Some(msg);
                    }
                    Err(_e) => {
                        //Bad packet, report no packet with data read
                        //println!("Bad packet: {}", _e);
                    }
                }
                self.scratch_offset = 0;
            }
        }

        Ok((read, res))
    }
}

#[cfg(test)]
mod test {
    use crate::Message;
    use super::{Link, MAX_PACKET_SIZE};

    fn echo_test(msg: Message) {
        let mut buf = [0u8; MAX_PACKET_SIZE];
        let mut link = Link::new();
        let size = link.encode(&msg, &mut buf).unwrap();
        let (_size, rx) = link.decode(&buf[..size]).unwrap();
        assert_eq!(msg, rx.unwrap());
    }

    fn multi_message_encode(msgs: &Vec<Message>, link: &mut Link) -> (usize, Vec<u8>) {
        let mut buf = vec![0u8; msgs.len() * MAX_PACKET_SIZE];
        println!("Buffer size: {}", buf.len());
        let mut offset = 0;
        for msg in msgs {
            let size = link.encode(&msg, &mut buf[offset..]).unwrap();
            println!("SIZE: {} Offset: {} Buf: {:?}", size, offset, &buf[offset..size+offset]);
            offset += size;
        }
        (offset, buf)
    }

    fn multi_message_decode(msgs: &Vec<Message>, link: &mut Link, buf: &[u8]) {
        let mut msgindex = 0;
        let mut offset = 0;
        loop {
            let (size, rx) = link.decode(&buf[offset..]).unwrap();
            println!("Buf {:?}", &buf[offset..]);
            if size == 0 {
                break;
            }
            if rx.is_some() {
                assert_eq!(msgs[msgindex], rx.unwrap());
                msgindex += 1;
            }
            offset += size;
        }
    }

    #[test]
    fn say_hello() {
        echo_test(Message::Hello);
    }

    #[test]
    fn large_buffer() {
        echo_test(Message::log(b"Hello, World"))
    }

    #[test]
    fn multiple_message() {
        let msgs = vec![Message::Hello, Message::Hello];
        let mut link = Link::new();
        let (length, buf) = multi_message_encode(&msgs, &mut link);
        multi_message_decode(&msgs, &mut link, &buf[..length]);
    }

    #[test]
    fn bad_message() {
        let mut offset = 0;
        let input = vec![
            192, 111, 73, 111, 118, 118, 111, 192,
            192, 101, 72, 101, 108, 108, 111, 192];
        let mut link = Link::new();
        let (sz, msg) = link.decode(&input).unwrap();
        offset += sz;
        assert!(msg.is_none());
        let (sz, msg) = link.decode(&input[offset..]).unwrap();
        offset += sz;
        assert!(msg.is_none());
        let (_, msg) = link.decode(&input[offset..]).unwrap();
        assert_eq!(Message::Hello, msg.unwrap());
    }

    #[test]
    fn partial_decodes() {

        let input = vec![192, 101, 72, 101, 108, 108, 111, 192];
        let mut link = Link::new();
        let (sz, msg) = link.decode(&input[..2]).unwrap();
        assert!(msg.is_none());
        assert_eq!(sz, 2);
        let (sz, msg) = link.decode(&input[sz..]).unwrap();
        assert!(msg.is_some());
        assert_eq!(sz, 6);
    }

}

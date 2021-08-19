use serde::{Deserialize, Serialize};
use serde_big_array::big_array;
use serde_cbor::{
    ser::SliceWrite,
    Serializer,
    error::Error as CborError,
    de::from_mut_slice,
};

big_array! { BigArray; }


#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Message {
    Nop,
    Hello,
    HelloAck,
    Log(InternalBuffer),
    AccelReq,
    Accel(f32, f32, f32),
    MagReq,
    Mag(i16, i16, i16),
}

impl Default for Message {
    fn default() -> Message {
        Message::Nop
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InternalBuffer {
    #[serde(with = "BigArray")]
    b: [u8; 128],
}

impl<T> From<T> for InternalBuffer
where
    T: AsRef<[u8]>,
{
    fn from(t: T) -> InternalBuffer {
        let tref = t.as_ref();
        if tref.len() > 128 {
            panic!("Buffer conversion too large!");
        }
        let mut b = [0u8; 128];
        b[..tref.len()].copy_from_slice(tref);
        InternalBuffer {
            b,
        }
    }
}

impl Message {
    pub const MAX_SIZE: usize = 256;

    pub fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, CborError> {
        let mut ser = Serializer::new(SliceWrite::new(&mut buf[..]));
        self.serialize(&mut ser)?;
        Ok(ser.into_inner().bytes_written())
    }

    pub fn from_bytes(buf: &mut [u8]) -> Result<Message, CborError> {
        from_mut_slice(buf)
    }

    pub fn log<T: AsRef<[u8]>>(t: T) -> Self {
        Message::Log(InternalBuffer::from(t))
    }

}


#[cfg(test)]
mod tests {
    use super::{Message, InternalBuffer};
    use serde::Serialize;
    use serde_cbor::Serializer;
    use serde_cbor::ser::SliceWrite;

    fn get_size(msg: &Message, buf: &mut [u8]) -> usize {
        let writer = SliceWrite::new(&mut buf[..]);
        let mut ser = Serializer::new(writer);
        msg.serialize(&mut ser).unwrap();

        let writer = ser.into_inner();
        writer.bytes_written()
    }

    #[test]
    fn check_size() {
        let mut buf = [0u8; Message::MAX_SIZE];

        assert!(std::mem::size_of::<Message>() < Message::MAX_SIZE);
        assert!(get_size(&Message::Nop, &mut buf) < Message::MAX_SIZE);
        assert!(get_size(&Message::Hello, &mut buf) < Message::MAX_SIZE);
        assert!(get_size(&Message::Log(InternalBuffer{b: [0u8; 128]}), &mut buf) < Message::MAX_SIZE);
    }

    #[test]
    fn tx_rx() {
        let msg = Message::Hello;
        let mut buf = [0u8; Message::MAX_SIZE];
        let size = msg.write_bytes(&mut buf).unwrap();
        let rx_msg = Message::from_bytes(&mut buf[..size]).unwrap();
        assert_eq!(msg, rx_msg);
    }
}


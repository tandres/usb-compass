use serde::{Deserialize, Serialize};
use serde_big_array::big_array;

big_array! { BigArray; }


#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum Message {
    Nop,
    Hello,
    Log(InternalBuffer),
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct InternalBuffer {
    #[serde(with = "BigArray")]
    b: [u8; 128],
}

impl Message {
    pub const MAX_SIZE: usize = 256;
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
        msg.serialize(&mut ser);

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
}


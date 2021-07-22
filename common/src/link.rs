use crate::message::Message;
use embedded_hal::serial::{Read, Write};
use serde::{Deserialize, Serialize};
use serde_cbor::{
    ser::SliceWrite,
    Serializer,
    error::Error as CborError,
    de::from_mut_slice,
};
use usb_device::class_prelude::*;

const HEADER_SIZE: usize = 4; //Start, size(u16), c
const FRAME_DELIM: u8 = 0xFF;

#[derive(Debug)]
pub enum LinkError<E> {
    Cbor(CborError),
    WouldBlock,
    Serial(E),
}

impl<E> From<CborError> for LinkError<E> {
    fn from(e: CborError) -> LinkError<E> {
        LinkError::Cbor(e)
    }
}

impl<E> From<nb::Error<E>> for LinkError<E> {
    fn from(e: nb::Error<E>) -> LinkError<E> {
        match e {
            nb::Error::Other(e) => {
                LinkError::Serial(e)
            }
            nb::Error::WouldBlock => {
                LinkError::WouldBlock
            }
        }
    }
}

type Result<T, E> = core::result::Result<T, LinkError<E>>;


const BUFFER_SIZE: usize = 100;
pub struct Link<T> {
    inner: T,
}

impl<T, ER, EW> Link<T>
where
    T: Read<u8, Error = ER> + Write<u8, Error = EW>,
    LinkError<ER>: From<nb::Error<ER>>,
    LinkError<EW>: From<nb::Error<EW>>,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner,
        }
    }

    pub fn send(&mut self, message: &Message) -> Result<(), EW> {
        let mut buf = [0u8; Message::MAX_SIZE];
        let mut ser = Serializer::new(SliceWrite::new(&mut buf[..]));
        message.serialize(&mut ser)?;

        let size = ser.into_inner().bytes_written();
        for i in 0..size {
            self.inner.write(buf[i])?;
        }
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<Message, ER> {
        let mut buf = [0u8; Message::MAX_SIZE];
        let mut index = 0;
        loop {
            match self.inner.read() {
                Ok(d) => {
                    buf[index] = d;
                    index += 1;
                },
                Err(nb::Error::WouldBlock) => {
                    break;
                },
                Err(_e) => {
                    panic!("Errored before my time!");
                }
            }
        }
        let msg = from_mut_slice(&mut buf[..index])?;
        Ok(msg)
    }
}

impl<T, B> UsbClass<B> for Link<T>
where
    T: UsbClass<B>,
    B: UsbBus,
{
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {
        self.inner.get_configuration_descriptors(writer)
    }

    fn reset(&mut self) {
         self.inner.reset()
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        self.inner.endpoint_in_complete(addr)
    }

    fn control_in(&mut self, xfer: ControlIn<B>) { self.inner.control_in(xfer); }

    fn control_out(&mut self, xfer: ControlOut<B>) { self.inner.control_out(xfer); }
}

#[cfg(test)]
mod test {
    use ringbuffer::{ConstGenericRingBuffer, RingBufferWrite, RingBufferRead, RingBuffer};
    use crate::Message;
    use super::Link;
    use embedded_hal::serial::{Read, Write};
    const BSIZE: usize = 256;
    struct FakeSerial {
        buf: ConstGenericRingBuffer<u8, BSIZE>,
    }

    impl FakeSerial {
        fn new() -> Self {
            Self {
                buf: ConstGenericRingBuffer::<u8, BSIZE>::new(),
            }
        }
    }

    impl Read<u8> for FakeSerial {
        type Error = ();

        fn read(&mut self) -> nb::Result<u8, Self::Error> {
            if self.buf.is_empty() {
                Err(nb::Error::WouldBlock)
            } else {
                Ok(self.buf.dequeue().unwrap())
            }
        }
    }

    impl Write<u8> for FakeSerial {
        type Error = ();
        fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
            if self.buf.is_full() {
                Err(nb::Error::WouldBlock)
            } else {
                self.buf.push(word);
                Ok(())
            }
        }

        fn flush(&mut self) -> nb::Result<(), Self::Error> {
            //nop
            Ok(())
        }
    }

    #[test]
    fn say_hello() {
        let msg = Message::Hello;
        let mut link = Link::new(FakeSerial::new());
        link.send(&msg);
        let mut rx_msg = link.try_recv().unwrap();
        assert_eq!(msg, rx_msg);
    }
}



#![cfg_attr(not(feature = "std"), no_std)]
pub mod usb {
    pub const VENDOR_ID: u16 = 0x1209;
    pub const PROD_ID: u16 = 0x0001;
}

pub mod link;
pub mod message;
pub mod message_queue;

pub use link::Link;
pub use message::Message;
pub use message_queue::MessageQueue;

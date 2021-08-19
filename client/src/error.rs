use thiserror::Error;
use common::link::LinkError;
use common::Message;

pub type Result<T> = std::result::Result<T, CompError>;

#[derive(Debug, Error)]
pub enum CompError {
    #[error("Usb Error: {error}")]
    UsbError {
        #[from]
        error: rusb::Error,
    },
    #[error("Link Error: {0}")]
    LinkError(String),
    #[error("Sync Send Error {error}")]
    SendError {
        #[from]
        error: std::sync::mpsc::SendError<Message>,
    },
    #[error("Sync Recv Error {error}")]
    RecvError {
        #[from]
        error: std::sync::mpsc::RecvError,
    },
    #[error("Sync TryRecv Error {error}")]
    TryRecvError {
        #[from]
        error: std::sync::mpsc::TryRecvError,
    }
}

impl From<LinkError> for CompError {
    fn from(error: LinkError) -> CompError {
        CompError::LinkError(format!("{:?}", error))
    }
}

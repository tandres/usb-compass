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
    #[error("Tokio Send Error {error}")]
    TokioSendError {
        #[from]
        error: tokio::sync::broadcast::error::SendError<Message>,
    },
    #[error("Tokio Recv Error {error}")]
    TokioRecvError {
        #[from]
        error: tokio::sync::broadcast::error::RecvError,
    },
    #[error("Tokio TryRecv Error {error}")]
    TokioTryRecvError {
        #[from]
        error: tokio::sync::broadcast::error::TryRecvError,
    }
}

impl From<LinkError> for CompError {
    fn from(error: LinkError) -> CompError {
        CompError::LinkError(format!("{:?}", error))
    }
}

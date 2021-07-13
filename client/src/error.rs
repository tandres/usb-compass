use thiserror::Error;

pub type Result<T> = std::result::Result<T, CompError>;

#[derive(Debug, Error)]
pub enum CompError {
    #[error("Usb Error: {error}")]
    UsbError {
        #[from]
        error: libusb::Error,
    },
}

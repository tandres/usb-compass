
pub mod usb {
    pub const VENDOR_ID: u16 = 0x1209;
    pub const PROD_ID: u16 = 0x0001;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

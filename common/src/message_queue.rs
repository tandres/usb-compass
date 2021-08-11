use crate::Message;

#[derive(Debug)]
pub enum Error {
    Full,
}

pub struct MessageManager {
    count: usize,
    read: usize,
    capacity: usize,
    buf: [Message; 10],
}

impl MessageManager {
    pub fn new() -> MessageManager {
        let buf = arr_macro::arr![Message::default(); 10];
        MessageManager {
            count: 0,
            read: 0,
            capacity: buf.len(),
            buf,
        }
    }

    pub fn enqueue(&mut self, msg: &Message) -> Result<(), Error> {
        if self.count == self.capacity {
            Err(Error::Full)
        } else {
            self.count += 1;
            self.buf[self.next(self.read, self.count)] = msg.clone();
            Ok(())
        }
    }

    pub fn dequeue(&mut self) -> Option<Message> {
        if self.count == 0 {
            None
        } else {
            self.count -= 1;
            let readloc = self.read;
            self.read = self.next(self.read, 1);
            Some(self.buf[readloc].clone())
        }
    }

    fn next(&self, x: usize, a: usize) -> usize {
        x + a % self.capacity
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn available_empty(&self) -> usize {
        self.capacity - self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity() {
        let mut mm = MessageManager::new();
        let msg = Message::Nop;
        for _ in 0..mm.capacity() {
            mm.enqueue(&msg).unwrap();
        }
        assert!(mm.enqueue(&msg).is_err());

        for _ in 0..mm.capacity() {
            let _ = mm.dequeue().unwrap();
        }

        assert!(mm.dequeue().is_none());
    }
}

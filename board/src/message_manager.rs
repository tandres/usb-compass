use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use common::{Message, MessageQueue};

static QUEUE: Mutex<RefCell<Option<MessageQueue>>> = Mutex::new(RefCell::new(None));

pub fn setup() {
    cortex_m::interrupt::free(|cs| {
        *QUEUE.borrow(cs).borrow_mut() = Some(MessageQueue::new());
    });
}

pub fn message_push(msg: Message) -> bool {
    let mut res = false;
    cortex_m::interrupt::free(|cs| {
        res = QUEUE.borrow(cs).borrow_mut().as_mut().unwrap().enqueue(&msg).is_ok();
    });
    res
}

pub fn message_pop() -> Option<Message> {
    let mut res = None;
    cortex_m::interrupt::free(|cs| {
        res = QUEUE.borrow(cs).borrow_mut().as_mut().unwrap().dequeue();
    });
    res
}

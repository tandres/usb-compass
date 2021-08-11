use common::MessageQueue;

static QUEUE: Mutex<RefCell<MessageQueue>> = Mutex::new(RefCell::new(MessageQueue::new()));



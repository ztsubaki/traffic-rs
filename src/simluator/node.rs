use crate::simluator::v2x::Message;

pub struct Node {}

impl Node {
    pub fn new() -> Self {
        Node {}
    }
    pub fn on_tick(&mut self) {}
    pub fn on_message(&mut self, msg: Message) {}
}

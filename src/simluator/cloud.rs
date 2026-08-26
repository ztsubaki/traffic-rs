use crate::simluator::v2x::Message;

pub struct Cloud {}

impl Cloud {
    pub fn new() -> Self {
        Cloud {}
    }
    pub fn on_tick(&mut self) {}
    pub fn on_message(&mut self, msg: Message) {}
}

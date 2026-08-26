use crate::simluator::{net::TrafficLightID, net::VehicleID};
use std::any::Any;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

pub struct Channel {
    queue: BinaryHeap<Reverse<Message>>,
}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            queue: BinaryHeap::new(),
        }
    }
    pub fn message_count(&self) -> usize {
        self.queue.len()
    }
    pub fn latset_time(&self) -> Option<usize> {
        self.queue.peek().map(|mesg| mesg.0.time_arrive)
    }
    pub fn pop(&mut self) -> Option<Message> {
        self.queue.pop().map(|m| m.0)
    }
}

pub struct Message {
    pub from: Communicator,
    pub to: Communicator,
    pub content: Box<dyn Any>,
    pub time_send: usize,
    pub time_arrive: usize,
}

impl Message {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        channel: &mut Channel,
        content: Box<dyn Any>,
        from: Communicator,
        to: Communicator,
        time_send: usize,
    ) {
        channel.queue.push(Reverse(Message {
            from,
            to,
            content,
            time_send,
            time_arrive: time_send + 1,
        }));
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        (self.time_arrive, self.time_send) == (other.time_arrive, other.time_send)
    }
}

impl Eq for Message {}

impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Message {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.time_arrive, self.time_send).cmp(&(other.time_arrive, other.time_send))
    }
}

#[allow(clippy::upper_case_acronyms)]
pub enum Communicator {
    Cloud,
    Edge(TrafficLightID),
    RSU(TrafficLightID),
    Vehicle(VehicleID),
}

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
        match self.queue.peek() {
            Some(mesg) => Some(mesg.0.time_arrive),
            None => None,
        }
    }
    pub fn pop(&mut self) -> Option<Message> {
        self.queue.pop().map(|m| m.0)
    }
}

pub struct Message {
    from: Communicator,
    to: Communicator,
    content: Box<dyn Any>,
    time_send: usize,
    time_arrive: usize,
}

impl Message {
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

pub enum Communicator {
    Cloud,
    Edge(TrafficLightID),
    RSU,
    Vehicle(VehicleID),
}

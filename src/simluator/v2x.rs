use rand::{RngExt, rngs::ThreadRng};
use rand_distr::{Distribution, Normal};

use crate::conf::CONFIG;
use crate::simluator::{net::TrafficLightID, net::VehicleID};
use std::any::Any;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

pub struct Channel {
    queue: BinaryHeap<Reverse<Message>>,
    cloud_delay_generator: Normal<f32>,
    rng: ThreadRng,
    package_cout: Vec<usize>,
    package_total: usize,
    delay_total: usize,
    package_loss: usize,
}

impl Channel {
    pub fn new(cloud_delay_generator: Normal<f32>) -> Channel {
        Channel {
            queue: BinaryHeap::new(),
            cloud_delay_generator,
            rng: rand::rng(),
            package_cout: Vec::new(),
            package_total: 0,
            delay_total: 0,
            package_loss: 0,
        }
    }
    pub fn message_count(&self) -> usize {
        self.queue.len()
    }
    pub fn latset_time(&self) -> Option<usize> {
        self.queue.peek().map(|mesg| mesg.0.time_arrive)
    }
    pub fn push(&mut self, mesg: Message) {
        if let Communicator::RSU(tls_id) = mesg.to {
            while self.package_cout.len() <= tls_id.0 {
                self.package_cout.push(0);
            }
            self.package_cout[tls_id.0] += 1;
        }
        self.delay_total += mesg.time_arrive - mesg.time_send;
        self.package_total += 1;
        self.queue.push(Reverse(mesg));
    }
    pub fn pop(&mut self) -> Option<Message> {
        match self.queue.pop() {
            Some(mesg) => {
                if let Communicator::RSU(tls_id) = mesg.0.to {
                    while self.package_cout.len() <= tls_id.0 {
                        self.package_cout.push(0);
                    }
                    self.package_cout[tls_id.0] -= 1;
                }
                Some(mesg.0)
            }
            None => None,
        }
    }
    pub fn get_package_count(&self, tls_id: TrafficLightID) -> usize {
        if self.package_cout.len() < tls_id.0 {
            0
        } else {
            self.package_cout[tls_id.0]
        }
    }
    pub fn get_average_delay(&self) -> f64 {
        (self.delay_total as f64) / (self.package_total as f64)
    }
    pub fn loss_package(&mut self) {
        self.package_loss += 1;
    }
    pub fn get_package_loss(&self) -> f64 {
        (self.package_loss as f64) / (self.package_total as f64 + self.package_loss as f64)
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
        if matches!(from, Communicator::Cloud) || matches!(to, Communicator::Cloud) {
            let delay = channel
                .cloud_delay_generator
                .sample(&mut channel.rng)
                .round()
                .min(1.0) as usize;
            if !channel.rng.random_bool(CONFIG.cloud_package_loss_rate) {
                channel.push(Message {
                    from,
                    to,
                    content,
                    time_send,
                    time_arrive: time_send + delay,
                });
            } else {
                channel.loss_package();
            }
        } else if matches!(from, Communicator::Vehicle(_)) && matches!(to, Communicator::RSU(_)) {
            if let Communicator::RSU(tls_id) = to {
                if channel.get_package_count(tls_id) < CONFIG.rsu_bandwidth {
                    let delay = (CONFIG.rsu_delay_min as f64
                        + CONFIG.rsu_delay_k
                            / (1.0
                                - (channel.get_package_count(tls_id) as f64)
                                    / (CONFIG.rsu_bandwidth as f64)))
                        as usize;
                    channel.push(Message {
                        from,
                        to,
                        content,
                        time_send,
                        time_arrive: time_send + delay,
                    });
                } else {
                    channel.loss_package();
                }
            }
        } else {
            channel.push(Message {
                from,
                to,
                content,
                time_send,
                time_arrive: time_send + 1,
            });
        }
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
#[derive(Debug)]
pub enum Communicator {
    Cloud,
    Node(TrafficLightID),
    RSU(TrafficLightID),
    Vehicle(VehicleID),
}

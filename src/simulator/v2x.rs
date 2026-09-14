use pyo3::{Py, PyAny};
use rand::{RngExt, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use crate::conf::CONFIG;
use crate::net::TrafficLightID;
use crate::simulator::VehicleID;

pub struct Channel {
    queue: BinaryHeap<Reverse<Message>>,
    cloud_delay_generator: Normal<f32>,
    rng: StdRng,
    packets_per_rsu: Vec<usize>,
    delivered_packet_count: usize,
    delay_total: usize,
    lost_packet_count: usize,
}

impl Channel {
    pub fn new(cloud_delay_generator: Normal<f32>) -> Channel {
        Channel {
            queue: BinaryHeap::new(),
            cloud_delay_generator,
            rng: rand::make_rng(),
            packets_per_rsu: Vec::new(),
            delivered_packet_count: 0,
            delay_total: 0,
            lost_packet_count: 0,
        }
    }
    pub fn message_count(&self) -> usize {
        self.queue.len()
    }
    pub fn latest_time(&self) -> Option<usize> {
        self.queue.peek().map(|message| message.0.time_arrive)
    }
    pub fn push(&mut self, message: Message) {
        if let Communicator::RSU(traffic_light_id) = message.to {
            while self.packets_per_rsu.len() <= traffic_light_id.to_usize() {
                self.packets_per_rsu.push(0);
            }
            self.packets_per_rsu[traffic_light_id.to_usize()] += 1;
        }
        self.delay_total += message.time_arrive - message.time_send;
        self.delivered_packet_count += 1;
        self.queue.push(Reverse(message));
    }
    pub fn pop(&mut self) -> Option<Message> {
        match self.queue.pop() {
            Some(message) => {
                if let Communicator::RSU(traffic_light_id) = message.0.to {
                    while self.packets_per_rsu.len() <= traffic_light_id.to_usize() {
                        self.packets_per_rsu.push(0);
                    }
                    self.packets_per_rsu[traffic_light_id.to_usize()] -= 1;
                }
                Some(message.0)
            }
            None => None,
        }
    }
    pub fn queued_packet_count(&self, traffic_light_id: TrafficLightID) -> usize {
        if self.packets_per_rsu.len() <= traffic_light_id.to_usize() {
            0
        } else {
            self.packets_per_rsu[traffic_light_id.to_usize()]
        }
    }
    pub fn get_average_delay(&self) -> f64 {
        if self.delivered_packet_count == 0 {
            0.0
        } else {
            (self.delay_total as f64) / (self.delivered_packet_count as f64)
        }
    }
    pub fn record_packet_loss(&mut self) {
        self.lost_packet_count += 1;
    }
    pub fn packet_loss_rate(&self) -> f64 {
        if self.delivered_packet_count + self.lost_packet_count == 0 {
            0.0
        } else {
            (self.lost_packet_count as f64)
                / (self.delivered_packet_count as f64 + self.lost_packet_count as f64)
        }
    }
}

pub struct Message {
    pub from: Communicator,
    pub to: Communicator,
    pub content: Py<PyAny>,
    pub time_send: usize,
    pub time_arrive: usize,
}

impl Message {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        channel: &mut Channel,
        content: Py<PyAny>,
        from: Communicator,
        to: Communicator,
        time_send: usize,
    ) {
        if matches!(from, Communicator::Cloud) || matches!(to, Communicator::Cloud) {
            let delay = channel
                .cloud_delay_generator
                .sample(&mut channel.rng)
                .round()
                .max(1.0) as usize;
            if !channel.rng.random_bool(CONFIG.cloud_packet_loss_rate) {
                channel.push(Message {
                    from,
                    to,
                    content,
                    time_send,
                    time_arrive: time_send + delay,
                });
            } else {
                channel.record_packet_loss();
            }
        } else if matches!(from, Communicator::Vehicle(_)) && matches!(to, Communicator::RSU(_)) {
            if let Communicator::RSU(traffic_light_id) = to {
                if channel.queued_packet_count(traffic_light_id) < CONFIG.rsu_bandwidth {
                    let delay = (CONFIG.rsu_delay_min as f64
                        + CONFIG.rsu_delay_k
                            / (1.0
                                - (channel.queued_packet_count(traffic_light_id) as f64)
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
                    channel.record_packet_loss();
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

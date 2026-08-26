use crate::simluator::{
    net::{TrafficLightID, Vehicle, VehicleID},
    v2x::{Channel, Communicator, Message},
};
use std::collections::HashMap;

#[allow(clippy::upper_case_acronyms)]
pub struct RSU {
    data: Vec<HashMap<VehicleID, Record>>,
}

impl RSU {
    pub fn new() -> RSU {
        RSU { data: Vec::new() }
    }
    pub fn on_message(&mut self, msg: Message) {
        let vehicle = msg.content.downcast::<Vehicle>().unwrap();
        let time_send = msg.time_send;
        let time_arrive = msg.time_arrive;
        let from = msg.from;
        let to = msg.to;
        if let Communicator::Vehicle(vehicle_id) = from {
            if let Communicator::RSU(tls_id) = to {
                while self.data.len() <= tls_id.0 {
                    self.data.push(HashMap::new());
                }
                match self.data[tls_id.0].get(&vehicle_id) {
                    Some(rec) => {
                        if rec.time_send < time_send {
                            self.data[tls_id.0].insert(
                                vehicle_id,
                                Record {
                                    vehicle,
                                    time_send,
                                    time_arrive,
                                },
                            );
                        }
                    }
                    None => {
                        self.data[tls_id.0].insert(
                            vehicle_id,
                            Record {
                                vehicle,
                                time_send,
                                time_arrive,
                            },
                        );
                    }
                }
            } else {
                unreachable!();
            }
        } else {
            unreachable!();
        }
    }
    pub fn report(&mut self, channel: &mut Channel, time: usize) {
        for (tls_id, data) in self.data.iter_mut().enumerate() {
            let map = Box::new(std::mem::take(data));
            Message::new(
                channel,
                map,
                Communicator::RSU(TrafficLightID(tls_id)),
                Communicator::Node(TrafficLightID(tls_id)),
                time,
            );
        }
    }
}

struct Record {
    vehicle: Box<Vehicle>,
    time_send: usize,
    time_arrive: usize,
}

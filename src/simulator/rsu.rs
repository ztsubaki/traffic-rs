use pyo3::{IntoPyObject, Py, Python, pyclass};
use std::collections::HashMap;

use crate::{
    net::TrafficLightID,
    simulator::{
        Vehicle, VehicleID,
        v2x::{Channel, Communicator, Message},
    },
};

#[allow(clippy::upper_case_acronyms)]
pub struct RSU {
    data: Vec<HashMap<VehicleID, Record>>,
}

impl RSU {
    pub fn new() -> RSU {
        RSU { data: Vec::new() }
    }
    pub fn on_message(&mut self, msg: Message) {
        let vehicle = Python::attach(|py| msg.content.into_bound(py).cast_into().unwrap().unbind());
        let time_send = msg.time_send;
        let time_arrive = msg.time_arrive;
        let from = msg.from;
        let to = msg.to;
        if let Communicator::Vehicle(vehicle_id) = from
            && let Communicator::RSU(tls_id) = to
        {
            while self.data.len() <= tls_id.to_usize() {
                self.data.push(HashMap::new());
            }
            match self.data[tls_id.to_usize()].get(&vehicle_id) {
                Some(rec) => {
                    if rec.time_send < time_send {
                        self.data[tls_id.to_usize()].insert(
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
                    self.data[tls_id.to_usize()].insert(
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
    }
    pub fn report(&mut self, channel: &mut Channel, time: usize) {
        for (tls_id, data) in self.data.iter_mut().enumerate() {
            let map = std::mem::take(data);
            let ctx = Python::attach(|py| map.into_pyobject(py).unwrap().unbind().into_any());
            Message::new(
                channel,
                ctx,
                Communicator::RSU(TrafficLightID::from_usize(tls_id)),
                Communicator::Node(TrafficLightID::from_usize(tls_id)),
                time,
            );
        }
    }
}

#[pyclass]
struct Record {
    #[pyo3(get)]
    vehicle: Py<Vehicle>,
    #[pyo3(get)]
    time_send: usize,
    #[pyo3(get)]
    time_arrive: usize,
}

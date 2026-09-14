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
    pub fn on_message(&mut self, message: Message) -> Result<(), String> {
        let Message {
            from,
            to,
            content,
            time_send,
            time_arrive,
        } = message;
        if let (Communicator::Vehicle(vehicle_id), Communicator::RSU(traffic_light_id)) = (from, to)
        {
            let vehicle = Python::attach(|py| {
                content
                    .into_bound(py)
                    .cast_into::<Vehicle>()
                    .map(|vehicle| vehicle.unbind())
                    .map_err(|error| format!("RSU received an invalid vehicle payload: {error}"))
            })?;
            while self.data.len() <= traffic_light_id.to_usize() {
                self.data.push(HashMap::new());
            }
            match self.data[traffic_light_id.to_usize()].get(&vehicle_id) {
                Some(record) => {
                    if record.time_send < time_send {
                        self.data[traffic_light_id.to_usize()].insert(
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
                    self.data[traffic_light_id.to_usize()].insert(
                        vehicle_id,
                        Record {
                            vehicle,
                            time_send,
                            time_arrive,
                        },
                    );
                }
            }
            Ok(())
        } else {
            Err(
                "RSU received a message that was not addressed from a vehicle to an RSU."
                    .to_owned(),
            )
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

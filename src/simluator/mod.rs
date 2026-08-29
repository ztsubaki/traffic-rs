mod cloud;
mod net;
mod node;
mod rsu;
mod v2x;

use std::{collections::HashMap, ffi::CStr, ffi::CString, str::FromStr, sync::Arc};

use crate::simluator::net::{
    Edge, EdgeID, Junction, JunctionID, Lane, LaneID, TrafficLight, TrafficLightID,
};
use crate::sumo;
use crate::{
    Sumo,
    conf::CONFIG,
    simluator::{
        cloud::{Cloud, CloudBase},
        net::{Vehicle, VehicleID},
        node::Node,
        v2x::Communicator,
    },
};
use log::{error, trace};
use net::Net;
use node::NodeBase;
use pyo3::{Bound, IntoPyObject, PyResult, Python, pymethods, pymodule};
use pyo3::{
    Py, pyclass, pyfunction,
    types::{PyModule, PyModuleMethods},
    wrap_pyfunction,
};
use rand_distr::Normal;
use rsu::RSU;
use v2x::{Channel, Message};

pub struct Simluator {
    sumo: Sumo,
    net: Arc<Net>,
    rsu: RSU,
    v2x: Channel,
    pub time_begin: usize,
    pub time_end: usize,
    pub step: usize,
    pub step_current: usize,
    nodes: Vec<Node>,
    cloud: Cloud,
    vehicles: Vec<Vehicle>,
    vehicle_ids: HashMap<CString, VehicleID>,
}

impl Simluator {
    pub fn init(sumo: Sumo, cloud_delay_generator: Normal<f32>, module: Py<PyModule>) -> Simluator {
        let time_begin = sumo.time_begin().unwrap();
        let time_end = sumo.time_end().unwrap();
        let step = sumo.step_length().unwrap();
        let edges = sumo.get_edges().unwrap();
        let mut net = Net::new();
        let junctions = sumo.get_junctions().unwrap();
        for junction in junctions.as_slice() {
            net.new_junction(junction);
        }
        log::info!("Loaded {} junctions", net.junction_count());
        for edge in edges.as_slice() {
            net.new_edge(edge);
        }
        log::info!("Loaded {} edges", net.edge_count());
        let lanes = sumo.get_lanes().unwrap();
        for lane in lanes.as_slice() {
            net.new_lane(lane);
        }
        log::info!("Loaded {} lanes", net.lane_count());
        let trafficlights = sumo.get_trafficlights().unwrap();
        for trafficlight in trafficlights.as_slice() {
            net.new_trafficlight(trafficlight);
        }
        let tls_count = net.trafficlight_count();
        log::info!("Loaded {} trafficlights", tls_count);
        let links = sumo.get_links().unwrap();
        for link in links.as_slice() {
            net.new_link(link);
        }
        log::info!(
            "Loaded {} links with {} controlled links",
            net.link_count().0,
            net.link_count().1
        );
        let phases = sumo.get_phases().unwrap();
        for phase in phases.as_slice() {
            net.new_phase(phase);
        }
        log::info!("Loaded {} phases", net.phase_count());
        net.check().unwrap_or_else(|e| {
            error!("Check road net failed: {}", e);
            std::process::exit(1);
        });
        let net = Arc::new(net);
        let mut v2x = v2x::Channel::new(cloud_delay_generator);
        let mut nodes = Vec::<Node>::new();
        Python::attach(|py| {
            let node_class = match module.getattr(py, "Node") {
                Ok(r) => r,
                Err(e) => {
                    error!("Class Node not found: {}", e);
                    std::process::exit(1);
                }
            };
            for id in 0..tls_count {
                let net = Arc::clone(&net);
                let pynet = PyNet::new(net).into_pyobject(py).unwrap();
                nodes.push(match node_class.call1(py, (id, pynet)) {
                    Ok(r) => Node::new(r),
                    Err(e) => {
                        error!("Failed to init Node: {}", e);
                        std::process::exit(1);
                    }
                });
                nodes.last_mut().unwrap().on_create(&mut v2x);
            }
        });
        let mut cloud = Python::attach(|py| {
            let net = Arc::clone(&net);
            let pynet = PyNet::new(net).into_pyobject(py).unwrap();
            match match module.getattr(py, "Cloud") {
                Ok(r) => r,
                Err(e) => {
                    error!("Class Cloud not found: {}", e);
                    std::process::exit(1);
                }
            }
            .call1(py, (pynet,))
            {
                Ok(r) => Cloud::new(r),
                Err(e) => {
                    error!("Failed to init Cloud: {}", e);
                    std::process::exit(1);
                }
            }
        });
        cloud.on_create(&mut v2x);
        Simluator {
            sumo,
            net,
            rsu: RSU::new(),
            v2x,
            time_begin,
            time_end,
            step,
            step_current: 0,
            nodes,
            cloud,
            vehicles: Vec::new(),
            vehicle_ids: HashMap::new(),
        }
    }
    fn update_vehicle(&mut self, vehicle: &sumo::Vehicle) -> Result<VehicleID, String> {
        let lane_id_cstr = unsafe { CStr::from_ptr(vehicle.lane_id) };
        let lane_id = match self.net.get_lane_id(lane_id_cstr) {
            Some(id) => id,
            None => {
                return Err(format!(
                    "Can not found lane {:?} for vehicle {:?}",
                    lane_id_cstr, vehicle.id
                ));
            }
        };
        let next_tls_id_cstr = unsafe { CStr::from_ptr(vehicle.next_trafficlight) };
        let next_tls_id = if next_tls_id_cstr.is_empty() {
            None
        } else {
            match self.net.get_trafficlight_id(next_tls_id_cstr) {
                Some(id) => Some(id),
                None => {
                    return Err(format!(
                        "Can not found trafficlight {:?} for vehicle {:?}",
                        next_tls_id_cstr, vehicle.id
                    ));
                }
            }
        };
        let vehicle_id_cstr = unsafe { CStr::from_ptr(vehicle.id) };
        if vehicle_id_cstr.is_empty() {
            return Err("Empty vehicle id".to_string());
        }
        Ok(match self.vehicle_ids.get(vehicle_id_cstr) {
            Some(id) => {
                self.vehicles[id.0].lane = lane_id;
                self.vehicles[id.0].next_trafficlight = next_tls_id;
                self.vehicles[id.0].lane_position = vehicle.lane_position;
                self.vehicles[id.0].speed = vehicle.speed;
                self.vehicles[id.0].acceleration = vehicle.acceleration;
                self.vehicles[id.0].waiting_time = vehicle.waiting_time;
                self.vehicles[id.0].time_loss = vehicle.time_loss;
                self.vehicles[id.0].next_link_index = vehicle.next_link_index;
                *id
            }
            None => {
                self.vehicle_ids
                    .insert(vehicle_id_cstr.to_owned(), VehicleID(self.vehicles.len()));
                self.vehicles.push(Vehicle {
                    lane: lane_id,
                    next_trafficlight: next_tls_id,
                    lane_position: vehicle.lane_position,
                    speed: vehicle.speed,
                    acceleration: vehicle.acceleration,
                    waiting_time: vehicle.waiting_time,
                    time_loss: vehicle.time_loss,
                    next_link_index: vehicle.next_link_index,
                });
                VehicleID(self.vehicle_ids.len() - 1)
            }
        })
    }

    pub fn is_end(&self) -> bool {
        self.time_begin + self.step * self.step_current >= self.time_end
    }

    pub fn tick(&mut self) -> Result<(), String> {
        if self.is_end() {
            return Err(String::from_str("Simluation ended").unwrap());
        }
        let time = self.time_begin + self.step_current * self.step;
        let vehicles = self.sumo.get_vehicles()?;
        trace!("Collect {} vehicles", vehicles.as_slice().len());
        for veh in vehicles.as_slice() {
            let vehicle = self.update_vehicle(veh)?;
            self.send_v2x(&vehicle, time);
        }
        if self.step_current.is_multiple_of(CONFIG.rsu_report_interval) {
            self.rsu.report(&mut self.v2x, time);
        }
        trace!("Message quene count {}", self.message_count());
        for node in self.nodes.iter_mut() {
            node.on_tick(time, &mut self.v2x);
        }
        self.cloud.on_tick(time, &mut self.v2x);
        while self.v2x.latset_time().is_some() && self.v2x.latset_time().unwrap() < time + self.step
        {
            let msg = self.v2x.pop().unwrap();
            match msg.to {
                Communicator::Cloud => {
                    self.cloud.on_message(msg, &mut self.v2x);
                }
                Communicator::Node(tls_id) => {
                    self.nodes[tls_id.0].on_message(msg, &mut self.v2x);
                }
                Communicator::RSU(_) => {
                    self.rsu.on_message(msg);
                }
                Communicator::Vehicle(_) => {
                    unreachable!("Can not send message to vehicle in current version");
                }
            }
        }
        self.sumo.step()?;
        self.step_current += 1;
        Ok(())
    }
    pub fn send_v2x(&mut self, vehicle_id: &VehicleID, time: usize) {
        let vehicle = &self.vehicles[vehicle_id.0];
        let lane = &self.net.lanes[vehicle.lane.0];

        let ctx = Python::attach(|py| Py::new(py, vehicle.clone()).unwrap().into_any());

        for tls_id in &lane.trafficlight {
            let ctx_ref = Python::attach(|py| ctx.clone_ref(py));
            Message::new(
                &mut self.v2x,
                ctx_ref,
                Communicator::Vehicle(*vehicle_id),
                Communicator::RSU(*tls_id),
                time,
            );
        }
    }
    #[allow(unused)]
    pub fn vehicle_list(&self) -> Result<Vec<String>, String> {
        let vec = self.sumo.get_vehicles()?;
        let mut strings: Vec<String> = Vec::new();
        for vechicle in vec.as_slice() {
            strings.push(
                unsafe { CStr::from_ptr(vechicle.id) }
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        Ok(strings)
    }

    pub fn message_count(&self) -> usize {
        self.v2x.message_count()
    }

    pub fn v2x_stats(&self) -> (f64, f64) {
        (self.v2x.get_average_delay(), self.v2x.get_package_loss())
    }
}

#[pyfunction]
fn hello_world() {
    log::info!("Hello World from Python");
}
#[pymodule]
pub fn trafficrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NodeBase>()?;
    m.add_class::<CloudBase>()?;
    m.add_function(wrap_pyfunction!(hello_world, m)?)?;
    Ok(())
}

#[pyclass(frozen)]
pub struct PyNet {
    inner: Arc<Net>,
}

impl PyNet {
    pub fn new(net: Arc<Net>) -> Self {
        Self { inner: net }
    }
}

#[pymethods]
impl PyNet {
    fn get_trafficlight(&self, id: usize) -> Option<TrafficLight> {
        self.inner.get_trafficlight(TrafficLightID(id)).cloned()
    }
    fn get_lane(&self, id: usize) -> Option<Lane> {
        self.inner.get_lane(LaneID(id)).cloned()
    }
    fn get_edge(&self, id: usize) -> Option<Edge> {
        self.inner.get_edge(EdgeID(id)).cloned()
    }
    fn get_junction(&self, id: usize) -> Option<Junction> {
        self.inner.get_junction(JunctionID(id)).cloned()
    }
}

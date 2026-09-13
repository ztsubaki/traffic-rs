mod cloud;
mod node;
mod rsu;
mod v2x;

use log::{trace, warn};
use pyo3::{
    Bound, FromPyObject, IntoPyObject, Py, PyErr, PyResult, Python, pyclass, pyfunction, pymethods,
    pymodule,
    types::{PyModule, PyModuleMethods},
    wrap_pyfunction,
};
use rand_distr::Normal;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    str::FromStr,
    sync::{Arc, Mutex},
};

use crate::net::Net;
use crate::net::{
    Edge, EdgeID, Junction, JunctionID, Lane, LaneID, Signals, TrafficLight, TrafficLightID,
};
use crate::sumo;
use crate::{
    Sumo,
    conf::CONFIG,
    simulator::{
        cloud::{Cloud, CloudBase},
        node::Node,
        v2x::Communicator,
    },
};
use node::NodeBase;
use rsu::RSU;
use v2x::{Channel, Message};

pub struct Simulator {
    sumo: Mutex<Sumo>,
    net: Arc<Net>,
    rsu: Option<RSU>,
    v2x: Option<Channel>,
    pub time_begin: usize,
    pub time_end: usize,
    pub step: usize,
    pub step_current: usize,
    nodes: Option<Vec<Node>>,
    cloud: Option<Cloud>,
    vehicles: Vec<Vehicle>,
    vehicle_ids: HashMap<CString, VehicleID>,
    baseline: bool,
}

impl Simulator {
    pub fn init(
        sumo: Sumo,
        module: Option<Py<PyModule>>,
        net: Arc<Net>,
    ) -> Result<Simulator, String> {
        let time_begin = sumo.time_begin()?;
        let time_end = sumo.time_end()?;
        let step = sumo.step_length()?;
        let edges = sumo.get_edges()?;
        if let Some(module) = module {
            let v2x = v2x::Channel::new(
                match Normal::new(
                    CONFIG.cloud_delay_mean as f32,
                    CONFIG.cloud_delay_std_dev as f32,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(format!(
                            "Failed to create the cloud delay distribution: {}",
                            e
                        ));
                    }
                },
            );
            let mut nodes = Vec::<Node>::new();
            let node_class = match Python::attach(|py| module.as_ref().getattr(py, "Node")) {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!("Failed to access the Python Node class: {}", e));
                }
            };
            let tls_count = net.get_num_trafficlights();
            for id in 0..tls_count {
                let mut node = Node::init(
                    TrafficLightID::from_usize(id),
                    PyNet::new(Arc::clone(&net)),
                    &node_class,
                )?;
                node.on_create()?;
                nodes.push(node);
            }
            let cloud_class = match Python::attach(|py| module.as_ref().getattr(py, "Cloud")) {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!("Failed to access the Python Cloud class: {}", e));
                }
            };
            let mut cloud = Cloud::init(PyNet::new(Arc::clone(&net)), &cloud_class)?;
            cloud.on_create()?;
            Ok(Simulator {
                sumo: Mutex::new(sumo),
                net,
                rsu: Some(RSU::new()),
                v2x: Some(v2x),
                time_begin,
                time_end,
                step,
                step_current: 0,
                nodes: Some(nodes),
                cloud: Some(cloud),
                vehicles: Vec::new(),
                vehicle_ids: HashMap::new(),
                baseline: false,
            })
        } else {
            Ok(Simulator {
                sumo: Mutex::new(sumo),
                net,
                rsu: None,
                v2x: None,
                time_begin,
                time_end,
                step,
                step_current: 0,
                nodes: None,
                cloud: None,
                vehicles: Vec::new(),
                vehicle_ids: HashMap::new(),
                baseline: true,
            })
        }
    }
    fn update_vehicle(&mut self, vehicle: &sumo::Vehicle) -> Result<VehicleID, String> {
        let lane_id_cstr = unsafe { CStr::from_ptr(vehicle.lane_id) };
        let lane_id = match self.net.get_lane_id(lane_id_cstr) {
            Some(id) => id,
            None => {
                return Err(format!(
                    "Vehicle {:?} references an unknown lane {:?}",
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
                        "Vehicle {:?} references an unknown next traffic light {:?}",
                        next_tls_id_cstr, vehicle.id
                    ));
                }
            }
        };
        let vehicle_id_cstr = unsafe { CStr::from_ptr(vehicle.id) };
        if vehicle_id_cstr.is_empty() {
            return Err("Received a vehicle with an empty ID".to_string());
        }
        Ok(match self.get_vehicle_id(vehicle_id_cstr) {
            Some(id) => {
                let veh = self.get_vehicle_mut(id).unwrap();
                veh.lane = lane_id;
                veh.next_trafficlight = next_tls_id;
                veh.lane_position = vehicle.lane_position;
                veh.speed = vehicle.speed;
                veh.acceleration = vehicle.acceleration;
                veh.waiting_time = vehicle.waiting_time;
                veh.time_loss = vehicle.time_loss;
                veh.next_link_index = vehicle.next_link_index;
                id
            }
            None => {
                self.vehicle_ids.insert(
                    vehicle_id_cstr.to_owned(),
                    VehicleID::from_usize(self.vehicles.len()),
                );
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
                VehicleID::from_usize(self.vehicle_ids.len() - 1)
            }
        })
    }

    pub fn get_num_nodes(&self) -> usize {
        self.nodes.as_ref().map_or(0, |n| n.len())
    }
    pub fn is_end(&self) -> bool {
        self.time_begin + self.step * self.step_current >= self.time_end
    }

    pub fn tick(&mut self) -> Result<(), String> {
        if self.is_end() {
            return Err(String::from_str("Simulation has already ended").unwrap());
        }
        let time = self.time_begin + self.step_current * self.step;
        let vehicles = match self.sumo.lock() {
            Ok(r) => r,
            Err(e) => {
                return Err(e.to_string());
            }
        }
        .get_vehicles()?;
        trace!("Collected active vehicles: {}", vehicles.as_slice().len());
        if !self.baseline {
            for veh in vehicles.as_slice() {
                let vehicle = self.update_vehicle(veh)?;
                self.send_v2x(vehicle, time);
            }
            if self.step_current.is_multiple_of(CONFIG.rsu_report_interval) {
                self.rsu
                    .as_mut()
                    .unwrap()
                    .report(self.v2x.as_mut().unwrap(), time);
            }
            trace!("V2X message queue length: {}", self.message_count());
            for id in 0..self.get_num_nodes() {
                let tls_id = TrafficLightID::from_usize(id);
                if let Some(sig) =
                    self.nodes.as_mut().unwrap()[id].on_tick(time, self.v2x.as_mut().unwrap())?
                {
                    self.set_trafficlights(&sig, tls_id);
                };
            }
            self.cloud
                .as_mut()
                .unwrap()
                .on_tick(time, self.v2x.as_mut().unwrap())?;
            while self.v2x.as_ref().unwrap().latest_time().is_some()
                && self.v2x.as_ref().unwrap().latest_time().unwrap() < time + self.step
            {
                let msg = self.v2x.as_mut().unwrap().pop().unwrap();
                match msg.to {
                    Communicator::Cloud => {
                        self.cloud
                            .as_mut()
                            .unwrap()
                            .on_message(msg, self.v2x.as_mut().unwrap())?;
                    }
                    Communicator::Node(tls_id) => {
                        if self.get_num_nodes() <= tls_id.to_usize() {
                            warn!(
                                "Dropping V2X message addressed to unknown node: {:?}",
                                tls_id
                            );
                            continue;
                        }
                        if let Some(sig) = self.nodes.as_mut().unwrap()[tls_id.to_usize()]
                            .on_message(msg, self.v2x.as_mut().unwrap())?
                        {
                            self.set_trafficlights(&sig, tls_id);
                        };
                    }
                    Communicator::RSU(_) => {
                        self.rsu.as_mut().unwrap().on_message(msg);
                    }
                    Communicator::Vehicle(_) => {
                        unreachable!("Can not send message to vehicle in current version");
                    }
                }
            }
        } else {
            for veh in vehicles.as_slice() {
                let _ = self.update_vehicle(veh)?;
            }
        }
        let vehicles = match self.sumo.lock() {
            Ok(r) => r,
            Err(e) => {
                return Err(e.to_string());
            }
        }
        .step()?;
        self.step_current += 1;
        Ok(())
    }
    pub fn send_v2x(&mut self, vehicle_id: VehicleID, time: usize) {
        let vehicle = self.get_vehicle(vehicle_id).unwrap();
        let lane = self.net.get_lane(vehicle.lane).unwrap();

        let ctx = Python::attach(|py| Py::new(py, vehicle.clone()).unwrap().into_any());

        for tls_id in &lane.trafficlight {
            let ctx_ref = Python::attach(|py| ctx.clone_ref(py));
            Message::new(
                self.v2x.as_mut().unwrap(),
                ctx_ref,
                Communicator::Vehicle(vehicle_id),
                Communicator::RSU(*tls_id),
                time,
            );
        }
    }

    pub fn message_count(&self) -> usize {
        self.v2x.as_ref().unwrap().message_count()
    }

    pub fn v2x_stats(&self) -> (f64, f64) {
        (
            self.v2x.as_ref().unwrap().get_average_delay(),
            self.v2x.as_ref().unwrap().get_package_loss(),
        )
    }

    fn get_vehicle_id(&self, vehicle: &CStr) -> Option<VehicleID> {
        self.vehicle_ids.get(vehicle).copied()
    }

    fn get_vehicle(&self, vehicle_id: VehicleID) -> Option<&Vehicle> {
        if self.vehicles.len() > vehicle_id.to_usize() {
            Some(&self.vehicles[vehicle_id.to_usize()])
        } else {
            None
        }
    }

    fn get_vehicle_mut(&mut self, vehicle_id: VehicleID) -> Option<&mut Vehicle> {
        if self.vehicles.len() > vehicle_id.to_usize() {
            Some(&mut self.vehicles[vehicle_id.to_usize()])
        } else {
            None
        }
    }

    fn set_trafficlights(&self, sig: &Signals, trafficlight_id: TrafficLightID) {
        let sumo = self.sumo.lock().unwrap();
        if let Some(tls) = self.net.get_trafficlight(trafficlight_id) {
            let sig = sig.to_str();
            if sig.len() == tls.links.len() {
                if let Err(e) = sumo.set_trafficlight(&tls.id, &sig) {
                    log::warn!(
                        "Failed to set traffic light {} to state {}: {}",
                        tls.id,
                        sig,
                        e
                    );
                };
            } else {
                log::warn!(
                    "Rejected traffic light state for {}: received_length={}, expected_length={}",
                    tls.id,
                    sig.len(),
                    tls.links.len()
                );
            }
        } else {
            warn!(
                "Traffic light ID does not exist in the road network: {:?}",
                trafficlight_id
            );
        }
    }
}

#[pyfunction]
fn hello_world() {
    log::debug!("Python extension module loaded successfully");
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
    fn get_trafficlight(&self, id: TrafficLightID) -> Option<TrafficLight> {
        self.inner.get_trafficlight(id).cloned()
    }
    fn get_lane(&self, id: LaneID) -> Option<Lane> {
        self.inner.get_lane(id).cloned()
    }
    fn get_edge(&self, id: EdgeID) -> Option<Edge> {
        self.inner.get_edge(id).cloned()
    }
    fn get_junction(&self, id: JunctionID) -> Option<Junction> {
        self.inner.get_junction(id).cloned()
    }
}

#[derive(Clone)]
#[pyclass(frozen, skip_from_py_object)]
pub struct Vehicle {
    #[pyo3(get)]
    pub lane: LaneID,
    #[pyo3(get)]
    pub lane_position: f64,
    #[pyo3(get)]
    pub speed: f64,
    #[pyo3(get)]
    pub acceleration: f64,
    #[pyo3(get)]
    pub waiting_time: f64,
    #[pyo3(get)]
    pub time_loss: f64,
    #[pyo3(get)]
    pub next_trafficlight: Option<TrafficLightID>,
    #[pyo3(get)]
    pub next_link_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VehicleID(usize);

impl VehicleID {
    fn from_usize(value: usize) -> Self {
        Self(value)
    }

    fn to_usize(self) -> usize {
        self.0
    }
}
impl<'py> IntoPyObject<'py> for VehicleID {
    type Target = pyo3::types::PyInt;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_pyobject(py)
    }
}

impl FromPyObject<'_, '_> for VehicleID {
    type Error = PyErr;

    fn extract(
        obj: pyo3::prelude::Borrowed<'_, '_, pyo3::prelude::PyAny>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(obj.extract::<usize>()?))
    }
}

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
use serde::Serialize;
use std::time::Duration;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    sync::{Arc, Mutex},
    time::Instant,
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
    global_statistics: GlobalStatisticsCounters,
    baseline: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLightStatus {
    pub signals: Signals,
    pub links: Vec<TrafficLightLinkStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLightLinkStatus {
    pub link_index: usize,
    pub movements: Vec<TrafficLightMovementStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLightMovementStatus {
    pub source_lane_id: LaneID,
    pub via_lane_id: Option<LaneID>,
    pub target_lane_id: LaneID,
    pub incoming_vehicle_count: usize,
    pub incoming_queue_count: usize,
    pub incoming_mean_speed: f64,
    pub incoming_occupancy_percent: f64,
    pub incoming_waiting_time_seconds: f64,
    pub outgoing_vehicle_count: usize,
    pub outgoing_queue_count: usize,
    pub pressure: i64,
}

#[derive(Default)]
struct GlobalStatisticsCounters {
    current_time_millis: usize,
    active_vehicle_count: usize,
    loaded_vehicle_count: usize,
    departed_vehicle_count: usize,
    arrived_vehicle_count: usize,
    starting_teleport_count: usize,
    ending_teleport_count: usize,
    collision_count: usize,
    min_expected_vehicle_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatistics {
    pub current_time_millis: usize,
    pub active_vehicle_count: usize,
    pub observed_vehicle_count: usize,
    pub loaded_vehicle_count: usize,
    pub departed_vehicle_count: usize,
    pub arrived_vehicle_count: usize,
    pub starting_teleport_count: usize,
    pub ending_teleport_count: usize,
    pub collision_count: usize,
    pub min_expected_vehicle_count: usize,
    pub total_time_loss_seconds: f64,
    pub average_time_loss_seconds: f64,
    pub total_waiting_time_seconds: f64,
    pub average_waiting_time_seconds: f64,
}

impl Simulator {
    pub fn init(
        sumo: Sumo,
        module: Option<Py<PyModule>>,
        net: Arc<Net>,
    ) -> Result<Simulator, String> {
        if module.is_some() && CONFIG.rsu_report_interval == 0 {
            return Err("rsu_report_interval must be greater than zero milliseconds".to_owned());
        }
        let time_begin = sumo.time_begin()?;
        let time_end = sumo.time_end()?;
        let step = sumo.step_length()?;
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
                global_statistics: GlobalStatisticsCounters {
                    current_time_millis: time_begin,
                    ..Default::default()
                },
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
                global_statistics: GlobalStatisticsCounters {
                    current_time_millis: time_begin,
                    ..Default::default()
                },
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

    pub fn node_count(&self) -> usize {
        self.nodes.as_ref().map_or(0, Vec::len)
    }
    pub fn traffic_light_status(
        &self,
        traffic_light_id: TrafficLightID,
    ) -> Result<Option<TrafficLightStatus>, String> {
        let traffic_light = match self.net.get_trafficlight(traffic_light_id) {
            Some(traffic_light) => traffic_light,
            None => return Ok(None),
        };
        let sumo = self.sumo.lock().map_err(|error| error.to_string())?;
        let state = sumo.get_trafficlight_state(&traffic_light.id)?;
        let signals = Signals::from_str(&state)
            .map_err(|error| format!("SUMO returned an invalid traffic light state: {error}"))?;
        let links = traffic_light
            .links
            .iter()
            .enumerate()
            .map(|(link_index, movements)| {
                let movements = movements
                    .iter()
                    .map(|movement| {
                        let source_lane =
                            self.net.get_lane(movement.source_lane_id).ok_or_else(|| {
                                "Traffic light references an unknown source lane".to_string()
                            })?;
                        let target_lane =
                            self.net.get_lane(movement.target_lane_id).ok_or_else(|| {
                                "Traffic light references an unknown target lane".to_string()
                            })?;
                        let incoming = sumo.get_lane_traffic_metrics(&source_lane.id)?;
                        let outgoing = sumo.get_lane_traffic_metrics(&target_lane.id)?;
                        Ok(TrafficLightMovementStatus {
                            source_lane_id: movement.source_lane_id,
                            via_lane_id: movement.via_lane_id,
                            target_lane_id: movement.target_lane_id,
                            incoming_vehicle_count: incoming.vehicle_count,
                            incoming_queue_count: incoming.queued_vehicle_count,
                            incoming_mean_speed: incoming.mean_speed,
                            incoming_occupancy_percent: incoming.occupancy_percent,
                            incoming_waiting_time_seconds: incoming.waiting_time_seconds,
                            outgoing_vehicle_count: outgoing.vehicle_count,
                            outgoing_queue_count: outgoing.queued_vehicle_count,
                            pressure: incoming.queued_vehicle_count as i64
                                - outgoing.queued_vehicle_count as i64,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(TrafficLightLinkStatus {
                    link_index,
                    movements,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Some(TrafficLightStatus { signals, links }))
    }
    pub fn global_statistics(&self) -> GlobalStatistics {
        let observed_vehicle_count = self.vehicles.len();
        let total_time_loss_seconds = self.vehicles.iter().map(|vehicle| vehicle.time_loss).sum();
        let total_waiting_time_seconds = self
            .vehicles
            .iter()
            .map(|vehicle| vehicle.waiting_time)
            .sum();
        let divisor = observed_vehicle_count as f64;
        GlobalStatistics {
            current_time_millis: self.global_statistics.current_time_millis,
            active_vehicle_count: self.global_statistics.active_vehicle_count,
            observed_vehicle_count,
            loaded_vehicle_count: self.global_statistics.loaded_vehicle_count,
            departed_vehicle_count: self.global_statistics.departed_vehicle_count,
            arrived_vehicle_count: self.global_statistics.arrived_vehicle_count,
            starting_teleport_count: self.global_statistics.starting_teleport_count,
            ending_teleport_count: self.global_statistics.ending_teleport_count,
            collision_count: self.global_statistics.collision_count,
            min_expected_vehicle_count: self.global_statistics.min_expected_vehicle_count,
            total_time_loss_seconds,
            average_time_loss_seconds: if observed_vehicle_count == 0 {
                0.0
            } else {
                total_time_loss_seconds / divisor
            },
            total_waiting_time_seconds,
            average_waiting_time_seconds: if observed_vehicle_count == 0 {
                0.0
            } else {
                total_waiting_time_seconds / divisor
            },
        }
    }
    pub fn is_end(&self) -> bool {
        self.time_begin + self.step * self.step_current >= self.time_end
    }

    pub fn tick(&mut self) -> Result<(Duration, Duration, Duration, Duration), String> {
        let mut simulate_duration = Duration::new(0, 0);
        let mut script_duration = Duration::new(0, 0);
        let mut sumo_duration = Duration::new(0, 0);
        let global_start = Instant::now();
        if self.is_end() {
            return Err("The simulation has already ended.".to_owned());
        }
        let time = self.time_begin + self.step_current * self.step;
        let simulate_start = Instant::now();
        let vehicles = match self.sumo.lock() {
            Ok(r) => r,
            Err(e) => {
                return Err(e.to_string());
            }
        }
        .get_vehicles()?;
        simulate_duration += simulate_start.elapsed();
        trace!("Collected active vehicles: {}", vehicles.as_slice().len());
        if !self.baseline {
            let simulate_start = Instant::now();
            for sumo_vehicle in vehicles.as_slice() {
                let vehicle = self.update_vehicle(sumo_vehicle)?;
                self.send_v2x(vehicle, time);
            }
            simulate_duration += simulate_start.elapsed();
            let elapsed = time.saturating_sub(self.time_begin);
            if elapsed != 0
                && elapsed / CONFIG.rsu_report_interval
                    > elapsed.saturating_sub(self.step) / CONFIG.rsu_report_interval
            {
                self.rsu
                    .as_mut()
                    .unwrap()
                    .report(self.v2x.as_mut().unwrap(), time);
            }
            trace!("V2X message queue length: {}", self.message_count());
            let script_start = Instant::now();
            for node_index in 0..self.node_count() {
                let traffic_light_id = TrafficLightID::from_usize(node_index);
                if let Some(signal_state) = self.nodes.as_mut().unwrap()[node_index]
                    .on_tick(time, self.v2x.as_mut().unwrap())?
                {
                    self.set_traffic_light(&signal_state, traffic_light_id);
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
                    Communicator::Node(traffic_light_id) => {
                        if self.node_count() <= traffic_light_id.to_usize() {
                            warn!(
                                "Dropping V2X message addressed to unknown node: {:?}",
                                traffic_light_id
                            );
                            continue;
                        }
                        if let Some(signal_state) = self.nodes.as_mut().unwrap()
                            [traffic_light_id.to_usize()]
                        .on_message(msg, self.v2x.as_mut().unwrap())?
                        {
                            self.set_traffic_light(&signal_state, traffic_light_id);
                        };
                    }
                    Communicator::RSU(_) => {
                        self.rsu.as_mut().unwrap().on_message(msg)?;
                    }
                    Communicator::Vehicle(vehicle_id) => {
                        return Err(format!(
                            "V2X delivery to vehicle {vehicle_id:?} is not supported."
                        ));
                    }
                }
            }
            script_duration += script_start.elapsed();
        } else {
            let simulate_start = Instant::now();
            for sumo_vehicle in vehicles.as_slice() {
                self.update_vehicle(sumo_vehicle)?;
            }
            simulate_duration += simulate_start.elapsed();
        }
        let sumo_start = Instant::now();
        match self.sumo.lock() {
            Ok(r) => r,
            Err(e) => {
                return Err(e.to_string());
            }
        }
        .step()?;
        sumo_duration += sumo_start.elapsed();
        self.step_current += 1;
        let statistics = self
            .sumo
            .lock()
            .map_err(|e| e.to_string())?
            .get_simulation_statistics()?;
        self.global_statistics.current_time_millis = statistics.current_time_millis;
        self.global_statistics.active_vehicle_count = statistics.active_vehicle_count;
        self.global_statistics.loaded_vehicle_count += statistics.loaded_vehicle_count;
        self.global_statistics.departed_vehicle_count += statistics.departed_vehicle_count;
        self.global_statistics.arrived_vehicle_count += statistics.arrived_vehicle_count;
        self.global_statistics.starting_teleport_count += statistics.starting_teleport_count;
        self.global_statistics.ending_teleport_count += statistics.ending_teleport_count;
        self.global_statistics.collision_count += statistics.collision_count;
        self.global_statistics.min_expected_vehicle_count = statistics.min_expected_vehicle_count;
        Ok((
            simulate_duration,
            script_duration,
            sumo_duration,
            global_start.elapsed(),
        ))
    }
    pub fn send_v2x(&mut self, vehicle_id: VehicleID, time: usize) {
        let vehicle = self.get_vehicle(vehicle_id).unwrap();
        let lane = self.net.get_lane(vehicle.lane).unwrap();

        let ctx = Python::attach(|py| Py::new(py, vehicle.clone()).unwrap().into_any());

        for traffic_light_id in &lane.traffic_light_ids {
            let ctx_ref = Python::attach(|py| ctx.clone_ref(py));
            Message::new(
                self.v2x.as_mut().unwrap(),
                ctx_ref,
                Communicator::Vehicle(vehicle_id),
                Communicator::RSU(*traffic_light_id),
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
            self.v2x.as_ref().unwrap().packet_loss_rate(),
        )
    }

    fn get_vehicle_id(&self, vehicle: &CStr) -> Option<VehicleID> {
        self.vehicle_ids.get(vehicle).copied()
    }

    pub fn get_vehicle(&self, vehicle_id: VehicleID) -> Option<&Vehicle> {
        if self.vehicles.len() > vehicle_id.to_usize() {
            Some(&self.vehicles[vehicle_id.to_usize()])
        } else {
            None
        }
    }

    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    fn get_vehicle_mut(&mut self, vehicle_id: VehicleID) -> Option<&mut Vehicle> {
        if self.vehicles.len() > vehicle_id.to_usize() {
            Some(&mut self.vehicles[vehicle_id.to_usize()])
        } else {
            None
        }
    }

    fn set_traffic_light(&self, signal_state: &Signals, traffic_light_id: TrafficLightID) {
        let sumo = self.sumo.lock().unwrap();
        if let Some(traffic_light) = self.net.get_trafficlight(traffic_light_id) {
            let signal_state = signal_state.to_str();
            if signal_state.len() == traffic_light.links.len() {
                if let Err(error) = sumo.set_trafficlight(&traffic_light.id, &signal_state) {
                    log::warn!(
                        "Failed to set traffic light {} to state {}: {}",
                        traffic_light.id,
                        signal_state,
                        error
                    );
                };
            } else {
                log::warn!(
                    "Rejected traffic light state for {}: received_length={}, expected_length={}",
                    traffic_light.id,
                    signal_state.len(),
                    traffic_light.links.len()
                );
            }
        } else {
            warn!(
                "Traffic light ID does not exist in the road network: {:?}",
                traffic_light_id
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

#[derive(Clone, Serialize)]
#[pyclass(frozen, skip_from_py_object)]
pub struct Vehicle {
    #[pyo3(get)]
    #[serde(rename = "currentLaneId")]
    pub lane: LaneID,
    #[pyo3(get)]
    #[serde(rename = "lanePosition")]
    pub lane_position: f64,
    #[pyo3(get)]
    pub speed: f64,
    #[pyo3(get)]
    pub acceleration: f64,
    #[pyo3(get)]
    #[serde(rename = "waitingTime")]
    pub waiting_time: f64,
    #[pyo3(get)]
    #[serde(rename = "timeLoss")]
    pub time_loss: f64,
    #[pyo3(get)]
    #[serde(rename = "nextTrafficLightId")]
    pub next_trafficlight: Option<TrafficLightID>,
    #[pyo3(get)]
    #[serde(rename = "nextLinkIndex")]
    pub next_link_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VehicleID(usize);

impl VehicleID {
    pub fn from_usize(value: usize) -> Self {
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

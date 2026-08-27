mod cloud;
mod net;
mod node;
mod rsu;
mod v2x;

use std::{ffi::CStr, str::FromStr};

use crate::{
    Sumo,
    conf::CONFIG,
    simluator::{
        cloud::{Cloud, CloudBase},
        node::Node,
        v2x::Communicator,
    },
};
use log::{error, trace};
use net::Net;
use node::NodeBase;
use pyo3::{Bound, PyResult, Python, pymodule};
use pyo3::{
    Py, pyfunction,
    types::{PyModule, PyModuleMethods},
    wrap_pyfunction,
};
use rand_distr::Normal;
use rsu::RSU;
use v2x::Channel;

pub struct Simluator {
    sumo: Sumo,
    net: Net,
    rsu: RSU,
    v2x: Channel,
    pub time_begin: usize,
    pub time_end: usize,
    pub step: usize,
    pub step_current: usize,
    rsu_report: usize,
    nodes: Vec<Node>,
    cloud: Cloud,
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
                nodes.push(match node_class.call1(py, (id,)) {
                    Ok(r) => Node::new(r),
                    Err(e) => {
                        error!("Failed to init Node: {}", e);
                        std::process::exit(1);
                    }
                })
            }
        });
        let cloud = Python::attach(|py| {
            match match module.getattr(py, "Cloud") {
                Ok(r) => r,
                Err(e) => {
                    error!("Class Cloud not found: {}", e);
                    std::process::exit(1);
                }
            }
            .call0(py)
            {
                Ok(r) => Cloud::new(r),
                Err(e) => {
                    error!("Failed to init Cloud: {}", e);
                    std::process::exit(1);
                }
            }
        });
        Simluator {
            sumo,
            net,
            rsu: RSU::new(),
            v2x: Channel::new(cloud_delay_generator),
            time_begin,
            time_end,
            step,
            step_current: 0,
            rsu_report: 0,
            nodes,
            cloud,
        }
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
            let vehicle = self.net.update_vehicle(veh)?;
            self.net.send_v2x(&vehicle, &mut self.v2x, time);
        }
        if self.step_current.is_multiple_of(CONFIG.rsu_report_interval) {
            self.rsu_report = time;
            self.rsu.report(&mut self.v2x, time);
        }
        trace!("Message quene count {}", self.message_count());
        for node in self.nodes.iter_mut() {
            node.on_tick(time);
        }
        self.cloud.on_tick(time);
        while self.v2x.latset_time().is_some() && self.v2x.latset_time().unwrap() < time + self.step
        {
            let msg = self.v2x.pop().unwrap();
            match msg.to {
                Communicator::Cloud => {
                    self.cloud.on_message(msg);
                }
                Communicator::Node(tls_id) => {
                    self.nodes[tls_id.0].on_message(msg);
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
    let _ = m.add_function(wrap_pyfunction!(hello_world, m)?);
    Ok(())
}

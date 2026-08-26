mod cloud;
mod net;
mod node;
mod rsu;
mod v2x;

use std::{ffi::CStr, str::FromStr};

use crate::{
    Sumo,
    conf::CONFIG,
    simluator::{cloud::Cloud, node::Node, v2x::Communicator},
};
use log::{error, trace};
use net::Net;
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
    pub fn init(sumo: Sumo, cloud_delay_generator: Normal<f32>) -> Simluator {
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
        log::info!("Loaded {} trafficlights", net.trafficlight_count());
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
        while nodes.len() < net.trafficlight_count() {
            nodes.push(Node::new());
        }
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
            cloud: Cloud::new(),
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
        if time - self.rsu_report >= CONFIG.rsu_report_interval {
            self.rsu_report = time;
            self.rsu.report(&mut self.v2x, time);
        }
        trace!("Message quene count {}", self.message_count());
        for node in self.nodes.iter_mut() {
            node.on_tick();
        }
        self.cloud.on_tick();
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
                    unreachable!("Can not sed message to vehicle in current version");
                }
            }
        }
        self.sumo.step()?;
        self.step_current += 1;
        Ok(())
    }

    pub fn close(self) -> Result<(), String> {
        self.sumo.close()
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

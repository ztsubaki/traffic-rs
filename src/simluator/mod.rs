mod net;
mod v2x;

use std::{ffi::CStr, str::FromStr};

use crate::Sumo;
use net::Net;

pub struct Simluator {
    sumo: Sumo,
    net: Net,
    pub time_begin: usize,
    pub time_end: usize,
    pub step: usize,
    pub step_current: usize,
}

impl Simluator {
    pub fn init(sumo: Sumo) -> Simluator {
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
        Simluator {
            sumo,
            net,
            time_begin,
            time_end,
            step,
            step_current: 0,
        }
    }

    pub fn is_end(&self) -> bool {
        self.time_begin + self.step * self.step_current >= self.time_end
    }

    pub fn tick(&mut self) -> Result<(), String> {
        if self.is_end() {
            return Err(String::from_str("Simluation ended").unwrap());
        }
        self.sumo.step()?;
        self.step_current += 1;
        Ok(())
    }

    pub fn close(self) -> Result<(), String> {
        self.sumo.close()
    }

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
}

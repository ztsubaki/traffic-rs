use log::warn;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
};

use crate::sumo;

pub struct Net {
    edge_ids: HashMap<CString, EdgeID>,
    junction_ids: HashMap<CString, JunctionID>,
    lane_ids: HashMap<CString, LaneID>,
    trafficlight_ids: HashMap<CString, TrafficLightID>,
    edges: Vec<Edge>,
    junctions: Vec<Junction>,
    lanes: Vec<Lane>,
    trafficlights: Vec<TrafficLight>,
    num_links: usize,
    num_controlled_links: usize,
}

impl Net {
    pub fn new() -> Net {
        Net {
            edge_ids: HashMap::new(),
            junction_ids: HashMap::new(),
            lane_ids: HashMap::new(),
            trafficlight_ids: HashMap::new(),
            edges: Vec::new(),
            junctions: Vec::new(),
            lanes: Vec::new(),
            trafficlights: Vec::new(),
            num_links: 0,
            num_controlled_links: 0,
        }
    }
    pub fn new_junction(&mut self, junction: &sumo::Junction) -> Option<JunctionID> {
        let junction_id = unsafe { CStr::from_ptr(junction.id) };
        if junction_id.is_empty() {
            log::warn!("Try to insert an empty junction id");
            return None;
        }
        self.junctions.push(Junction {
            id: junction_id.to_string_lossy().to_string(),
        });
        self.junction_ids
            .insert(junction_id.to_owned(), JunctionID(self.junctions.len() - 1))
    }
    pub fn new_edge(&mut self, edge: &sumo::Edge) -> Option<EdgeID> {
        let edge_id = unsafe { CStr::from_ptr(edge.id) };
        let edge_name = unsafe { CStr::from_ptr(edge.name) };
        let from_junction_id = unsafe { CStr::from_ptr(edge.from) };
        let to_junction_id = unsafe { CStr::from_ptr(edge.to) };
        let from = self
            .junction_ids
            .get(from_junction_id)
            .map_or_else(|| JunctionID(usize::MAX), |v| *v);
        if from == JunctionID(usize::MAX) && !from_junction_id.is_empty() {
            warn!(
                "Not found junction {:?} for edge {:?}",
                from_junction_id, edge_id
            );
        }
        let to = self
            .junction_ids
            .get(to_junction_id)
            .map_or_else(|| JunctionID(usize::MAX), |v| *v);
        if to == JunctionID(usize::MAX) && !to_junction_id.is_empty() {
            warn!(
                "Not found junction {:?} for edge {:?}",
                to_junction_id, edge_id
            );
        }
        self.edges.push(Edge {
            id: edge_id.to_string_lossy().to_string(),
            name: edge_name.to_string_lossy().to_string(),
            from,
            to,
            lane_number: edge.lane_number,
        });
        self.edge_ids
            .insert(edge_id.to_owned(), EdgeID(self.edges.len() - 1))
    }
    pub fn new_lane(&mut self, lane: &sumo::Lane) -> Option<LaneID> {
        let lane_id = unsafe { CStr::from_ptr(lane.id) };
        let edge_id = unsafe { CStr::from_ptr(lane.edge_id) };
        let edge = self
            .edge_ids
            .get(edge_id)
            .map_or_else(|| EdgeID(usize::MAX), |v| *v);
        if edge == EdgeID(usize::MAX) && !edge_id.is_empty() {
            warn!("Not found edge {:?} for lane {:?}", edge_id, edge_id);
        }
        self.lanes.push(Lane {
            id: lane_id.to_string_lossy().to_string(),
            edge_id: edge,
            length: lane.length,
            links: Vec::new(),
        });
        self.lane_ids
            .insert(lane_id.to_owned(), LaneID(self.lanes.len() - 1))
    }
    pub fn new_trafficlight(
        &mut self,
        trafficlight: &sumo::TrafficLight,
    ) -> Option<TrafficLightID> {
        let trafficlight_id = unsafe { CStr::from_ptr(trafficlight.id) };
        if trafficlight_id.is_empty() {
            log::warn!("Try to insert an empty trafficlight id");
            return None;
        }
        self.trafficlights.push(TrafficLight {
            id: trafficlight_id.to_string_lossy().to_string(),
            links: Vec::new(),
        });
        self.trafficlight_ids.insert(
            trafficlight_id.to_owned(),
            TrafficLightID(self.trafficlights.len() - 1),
        )
    }
    pub fn new_link(&mut self, link: &sumo::Link) {
        let trafficlight_id = unsafe { CStr::from_ptr(link.trafficlight) };
        let from_id = unsafe { CStr::from_ptr(link.from_lane) };
        let from = self
            .lane_ids
            .get(from_id)
            .map_or_else(|| LaneID(usize::MAX), |v| *v);
        if from == LaneID(usize::MAX) {
            warn!("Not found lane {:?} for a link to be insert", from_id);
            return;
        }
        let via_id = unsafe { CStr::from_ptr(link.via_lane) };
        let via = self
            .lane_ids
            .get(via_id)
            .map_or_else(|| LaneID(usize::MAX), |v| *v);
        let to_id = unsafe { CStr::from_ptr(link.to_lane) };
        let to = self
            .lane_ids
            .get(to_id)
            .map_or_else(|| LaneID(usize::MAX), |v| *v);
        if from == LaneID(usize::MAX) {
            warn!("Not found lane {:?} for a link to be insert", to_id);
            return;
        }
        if trafficlight_id.is_empty() {
            self.lanes[from.0].links.push(Link { from, via, to });
            self.num_links += 1;
        } else {
            let trafficlight = self
                .trafficlight_ids
                .get(trafficlight_id)
                .map_or_else(|| TrafficLightID(usize::MAX), |v| *v);
            if trafficlight == TrafficLightID(usize::MAX) {
                warn!(
                    "Not found traffic light {:?} for a link to be insert",
                    trafficlight_id
                );
                return;
            }
            if self.trafficlights[trafficlight.0].links.len() != link.link_index {
                warn!(
                    "Mismatch link index and traffic light link length, please make sure links insert in order"
                );
                return;
            }
            self.trafficlights[trafficlight.0]
                .links
                .push(Link { from, via, to });
            self.num_links += 1;
            self.num_controlled_links += 1;
        }
    }
    pub fn edge_count(&self) -> usize {
        self.edge_ids.len()
    }
    pub fn junction_count(&self) -> usize {
        self.junction_ids.len()
    }
    pub fn lane_count(&self) -> usize {
        self.lane_ids.len()
    }
    pub fn trafficlight_count(&self) -> usize {
        self.trafficlight_ids.len()
    }
    pub fn link_count(&self) -> (usize, usize) {
        (self.num_links, self.num_controlled_links)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct JunctionID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaneID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrafficLightID(usize);

pub struct Edge {
    id: String,
    name: String,
    from: JunctionID,
    to: JunctionID,
    lane_number: usize,
}

pub struct Junction {
    id: String,
}

pub struct Lane {
    id: String,
    edge_id: EdgeID,
    length: f64,
    links: Vec<Link>,
}

pub struct TrafficLight {
    id: String,
    links: Vec<Link>,
}

pub struct Link {
    from: LaneID,
    via: LaneID,
    to: LaneID,
}

use log::warn;
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString},
    fmt::Debug,
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
    num_phases: usize,
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
            num_phases: 0,
        }
    }
    pub fn new_junction(&mut self, junction: &sumo::Junction) -> Option<JunctionID> {
        let junction_id = unsafe { CStr::from_ptr(junction.id) };
        if junction_id.is_empty() {
            log::warn!("Try to insert an empty junction id");
            return None;
        }
        if let Some(jid) = self.junction_ids.get(junction_id) {
            warn!(
                "Trying to insert junction {:?} id {:?} twice",
                junction_id, jid
            );
            return Some(*jid);
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
            .map_or_else(|| None, |v| Some(*v));
        if from.is_none() && !from_junction_id.is_empty() {
            warn!(
                "Not found junction {:?} for edge {:?}",
                from_junction_id, edge_id
            );
        }
        let to = self
            .junction_ids
            .get(to_junction_id)
            .map_or_else(|| None, |v| Some(*v));
        if to.is_none() && !to_junction_id.is_empty() {
            warn!(
                "Not found junction {:?} for edge {:?}",
                to_junction_id, edge_id
            );
        }
        if let Some(eid) = self.edge_ids.get(edge_id) {
            warn!("Trying to insert edge {:?} id {:?} twice", edge_id, eid);
            return Some(*eid);
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
            .map_or_else(|| None, |v| Some(*v));
        if edge.is_none() && !edge_id.is_empty() {
            warn!("Not found edge {:?} for lane {:?}", edge_id, lane_id);
        }
        if let Some(lid) = self.lane_ids.get(lane_id) {
            warn!("Trying to insert lane {:?} id {:?} twice", lane_id, lid);
            return Some(*lid);
        }
        self.lanes.push(Lane {
            id: lane_id.to_string_lossy().to_string(),
            edge_id: edge,
            length: lane.length,
            links: Vec::new(),
            trafficlight: HashSet::new(),
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
        if let Some(tid) = self.trafficlight_ids.get(trafficlight_id) {
            warn!(
                "Trying to insert trafficlight {:?} id {:?} twice",
                trafficlight_id, tid
            );
            return Some(*tid);
        }
        self.trafficlights.push(TrafficLight {
            id: trafficlight_id.to_string_lossy().to_string(),
            links: Vec::new(),
            phases: Vec::new(),
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
            .map_or_else(|| None, |v| Some(*v));
        if from.is_none() {
            warn!("Not found lane {:?} for a link to be insert", from_id);
            return;
        }
        let via_id = unsafe { CStr::from_ptr(link.via_lane) };
        let via = self.lane_ids.get(via_id).map_or_else(|| None, |v| Some(*v));
        let to_id = unsafe { CStr::from_ptr(link.to_lane) };
        let to = self.lane_ids.get(to_id).map_or_else(|| None, |v| Some(*v));
        if to.is_none() {
            warn!("Not found lane {:?} for a link to be insert", to_id);
            return;
        }
        if trafficlight_id.is_empty() {
            self.lanes[from.unwrap().0].links.push(Link {
                from: from.unwrap(),
                via,
                to: to.unwrap(),
            });
            self.num_links += 1;
        } else {
            let trafficlight = self
                .trafficlight_ids
                .get(trafficlight_id)
                .map_or_else(|| None, |v| Some(*v));
            if trafficlight.is_none() {
                warn!(
                    "Not found traffic light {:?} for a link to be insert",
                    trafficlight_id
                );
                return;
            }
            while self.trafficlights[trafficlight.unwrap().0].links.len() <= link.link_index {
                self.trafficlights[trafficlight.unwrap().0]
                    .links
                    .push(Vec::new());
            }
            self.trafficlights[trafficlight.unwrap().0].links[link.link_index].push(Link {
                from: from.unwrap(),
                via,
                to: to.unwrap(),
            });
            self.lanes[from.unwrap().0]
                .trafficlight
                .insert(trafficlight.unwrap());
            if let Some(v) = via {
                self.lanes[v.0].trafficlight.insert(trafficlight.unwrap());
            }
            self.lanes[via.unwrap().0]
                .trafficlight
                .insert(trafficlight.unwrap());
            self.num_controlled_links += 1;
        }
    }
    pub fn new_phase(&mut self, phase: &sumo::Phase) {
        let trafficlight_id = unsafe { CStr::from_ptr(phase.trafficlight) };
        if trafficlight_id.is_empty() {
            warn!("Empty Trafficlight when inserting a phase");
            return;
        }
        let trafficlight = self
            .trafficlight_ids
            .get(trafficlight_id)
            .map_or_else(|| None, |v| Some(*v));
        if trafficlight.is_none() {
            warn!(
                "Not found traffic light {:?} for a phase to be insert",
                trafficlight_id
            );
            return;
        }
        while self.trafficlights[trafficlight.unwrap().0].phases.len() <= phase.program_index {
            self.trafficlights[trafficlight.unwrap().0]
                .phases
                .push(Vec::new());
        }
        while self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index].len()
            <= phase.phase_index
        {
            self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index].push(Phase {
                sigs: Vec::new(),
                next: Vec::new(),
            });
        }
        if (!self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index]
            [phase.phase_index]
            .sigs
            .is_empty())
            && (!self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index]
                [phase.phase_index]
                .next
                .is_empty())
        {
            warn!(
                "Trying to insert phase {}-{} for trafficlight {:?} twice",
                phase.program_index, phase.phase_index, trafficlight_id
            );
            return;
        }
        let state: String = unsafe { CStr::from_ptr(phase.state) }
            .to_string_lossy()
            .into();
        let mut next = Vec::new();
        next.extend_from_slice(unsafe {
            std::slice::from_raw_parts(phase.next_indexs, phase.next_count)
        });
        let sigs = match Signal::from_str(&state) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to resolve phase state: {}", e);
                return;
            }
        };
        self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index]
            [phase.phase_index]
            .sigs = sigs;
        self.trafficlights[trafficlight.unwrap().0].phases[phase.program_index]
            [phase.phase_index]
            .next = next;
        self.num_phases += 1;
    }
    pub fn check(&self) -> Result<(), String> {
        for tls in &self.trafficlights {
            for program in &tls.phases {
                for phase in program {
                    if phase.sigs.is_empty() && phase.next.is_empty() {
                        return Err("Empty phase found".to_string());
                    }
                }
            }
        }
        Ok(())
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
    pub fn phase_count(&self) -> usize {
        self.num_phases
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EdgeID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct JunctionID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LaneID(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TrafficLightID(usize);

pub struct Edge {
    id: String,
    name: String,
    from: Option<JunctionID>,
    to: Option<JunctionID>,
    lane_number: usize,
}

pub struct Junction {
    id: String,
}

pub struct Lane {
    id: String,
    edge_id: Option<EdgeID>,
    length: f64,
    links: Vec<Link>,
    trafficlight: HashSet<TrafficLightID>,
}

pub struct TrafficLight {
    id: String,
    links: Vec<Vec<Link>>,
    phases: Vec<Vec<Phase>>,
}

pub struct Link {
    from: LaneID,
    via: Option<LaneID>,
    to: LaneID,
}

pub struct Phase {
    sigs: Vec<Signal>,
    next: Vec<usize>,
}

#[allow(non_camel_case_types)]
pub enum Signal {
    r,
    u,
    G,
    g,
    Y,
    y,
    o,
    O,
}

impl Signal {
    fn from_str(str: &str) -> Result<Vec<Signal>, String> {
        let mut vec = Vec::new();
        for c in str.chars() {
            match c {
                'r' => {
                    vec.push(Signal::r);
                }
                'u' => {
                    vec.push(Signal::u);
                }
                'G' => {
                    vec.push(Signal::G);
                }
                'g' => {
                    vec.push(Signal::g);
                }
                'Y' => {
                    vec.push(Signal::Y);
                }
                'y' => {
                    vec.push(Signal::y);
                }
                'o' => {
                    vec.push(Signal::o);
                }
                'O' => {
                    vec.push(Signal::O);
                }
                e => {
                    return Err(format!("Wrong signal char {}", e));
                }
            }
        }
        Ok(vec)
    }
    fn to_str(sigs: &[Signal]) -> String {
        let mut str = String::new();
        for sig in sigs {
            match sig {
                Signal::r => {
                    str.push('r');
                }
                Signal::u => {
                    str.push('u');
                }
                Signal::G => {
                    str.push('G');
                }
                Signal::g => {
                    str.push('g');
                }
                Signal::Y => {
                    str.push('Y');
                }
                Signal::y => {
                    str.push('y');
                }
                Signal::o => {
                    str.push('o');
                }
                Signal::O => {
                    str.push('O');
                }
            }
        }
        str
    }
}

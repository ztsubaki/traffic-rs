use pyo3::{Bound, FromPyObject, IntoPyObject, PyErr, pyclass};
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
    pub fn init(&mut self, sumo: &sumo::Sumo) -> Result<(), String> {
        let edges = sumo.get_edges()?;
        let junctions = sumo.get_junctions()?;
        for junction in junctions.as_slice() {
            self.insert_junction(junction)?;
        }
        log::info!(
            "Loaded road network junctions: {}",
            self.get_num_junctions()
        );
        for edge in edges.as_slice() {
            self.insert_edge(edge)?;
        }
        log::info!("Loaded road network edges: {}", self.get_num_edges());
        let lanes = sumo.get_lanes()?;
        for lane in lanes.as_slice() {
            self.insert_lane(lane)?;
        }
        log::info!("Loaded road network lanes: {}", self.get_num_lanes());
        let trafficlights = sumo.get_trafficlights()?;
        for trafficlight in trafficlights.as_slice() {
            self.insert_trafficlight(trafficlight)?;
        }
        let tls_count = self.get_num_trafficlights();
        log::info!("Loaded traffic lights: {}", tls_count);
        let links = sumo.get_links()?;
        for link in links.as_slice() {
            self.insert_link(link)?;
        }
        log::info!(
            "Loaded road network links: total={}, signal_controlled={}",
            self.get_num_links().0,
            self.get_num_links().1
        );
        let phases = sumo.get_phases()?;
        for phase in phases.as_slice() {
            self.insert_phase(phase)?;
        }
        log::info!("Loaded traffic signal phases: {}", self.get_num_phases());
        self.check()?;
        Ok(())
    }
    fn insert_junction(&mut self, junction: &sumo::Junction) -> Result<JunctionID, String> {
        let junction_id = unsafe { CStr::from_ptr(junction.id) };
        if junction_id.is_empty() {
            return Err("Cannot insert a junction with an empty ID".to_string());
        }
        if let Some(jid) = self.junction_ids.get(junction_id) {
            return Err(format!(
                "Duplicate junction ID {:?}; existing internal ID: {:?}",
                junction_id, jid
            ));
        }
        self.junctions.push(Junction {
            id: junction_id.to_string_lossy().to_string(),
        });
        let jid = JunctionID::from_usize(self.junctions.len() - 1);
        self.junction_ids
            .insert(junction_id.to_owned(), jid)
            .map_or_else(|| Ok(jid), |_| unreachable!())
    }
    fn insert_edge(&mut self, edge: &sumo::Edge) -> Result<EdgeID, String> {
        let edge_id = unsafe { CStr::from_ptr(edge.id) };
        let edge_name = unsafe { CStr::from_ptr(edge.name) };
        let from_junction_id = unsafe { CStr::from_ptr(edge.from) };
        let to_junction_id = unsafe { CStr::from_ptr(edge.to) };
        let from = self.get_junction_id(from_junction_id);
        if from.is_none() && !from_junction_id.is_empty() {
            return Err(format!(
                "Edge {:?} references an unknown source junction {:?}",
                edge_id, from_junction_id
            ));
        }
        let to = self.get_junction_id(to_junction_id);
        if to.is_none() && !to_junction_id.is_empty() {
            return Err(format!(
                "Edge {:?} references an unknown destination junction {:?}",
                edge_id, to_junction_id
            ));
        }
        if let Some(eid) = self.edge_ids.get(edge_id) {
            return Err(format!(
                "Duplicate edge ID {:?}; existing internal ID: {:?}",
                edge_id, eid
            ));
        }
        self.edges.push(Edge {
            id: edge_id.to_string_lossy().to_string(),
            name: edge_name.to_string_lossy().to_string(),
            from,
            to,
            lane_number: edge.lane_number,
        });
        let eid = EdgeID::from_usize(self.edges.len() - 1);
        self.edge_ids
            .insert(edge_id.to_owned(), eid)
            .map_or_else(|| Ok(eid), |_| unreachable!())
    }
    fn insert_lane(&mut self, lane: &sumo::Lane) -> Result<LaneID, String> {
        let lane_id = unsafe { CStr::from_ptr(lane.id) };
        let edge_id = unsafe { CStr::from_ptr(lane.edge_id) };
        let edge = self.get_edge_id(edge_id);
        if edge.is_none() && !edge_id.is_empty() {
            return Err(format!(
                "Lane {:?} references an unknown edge {:?}",
                lane_id, edge_id
            ));
        }
        if let Some(lid) = self.lane_ids.get(lane_id) {
            return Err(format!(
                "Duplicate lane ID {:?}; existing internal ID: {:?}",
                lane_id, lid
            ));
        }
        self.lanes.push(Lane {
            id: lane_id.to_string_lossy().to_string(),
            edge_id: edge,
            length: lane.length,
            links: Vec::new(),
            trafficlight: HashSet::new(),
        });
        let lid = LaneID::from_usize(self.lanes.len() - 1);
        self.lane_ids
            .insert(lane_id.to_owned(), lid)
            .map_or_else(|| Ok(lid), |_| unreachable!())
    }
    fn insert_trafficlight(
        &mut self,
        trafficlight: &sumo::TrafficLight,
    ) -> Result<TrafficLightID, String> {
        let trafficlight_id = unsafe { CStr::from_ptr(trafficlight.id) };
        if trafficlight_id.is_empty() {
            return Err("Cannot insert a traffic light with an empty ID".to_string());
        }
        if let Some(tid) = self.trafficlight_ids.get(trafficlight_id) {
            return Err(format!(
                "Duplicate traffic light ID {:?}; existing internal ID: {:?}",
                trafficlight_id, tid
            ));
        }
        self.trafficlights.push(TrafficLight {
            id: trafficlight_id.to_string_lossy().to_string(),
            links: Vec::new(),
            phases: Vec::new(),
        });
        let tid = TrafficLightID::from_usize(self.trafficlights.len() - 1);
        self.trafficlight_ids
            .insert(trafficlight_id.to_owned(), tid)
            .map_or_else(|| Ok(tid), |_| unreachable!())
    }
    fn insert_link(&mut self, link: &sumo::Link) -> Result<(), String> {
        let trafficlight_id = unsafe { CStr::from_ptr(link.trafficlight) };
        let from_id = unsafe { CStr::from_ptr(link.from_lane) };
        let from = self.get_lane_id(from_id);
        if from.is_none() {
            return Err(format!(
                "Link references an unknown source lane {:?}",
                from_id
            ));
        }
        let via_id = unsafe { CStr::from_ptr(link.via_lane) };
        let via = self.get_lane_id(via_id);
        let to_id = unsafe { CStr::from_ptr(link.to_lane) };
        let to = self.get_lane_id(to_id);
        if to.is_none() {
            return Err(format!(
                "Link references an unknown destination lane {:?}",
                to_id
            ));
        }
        if trafficlight_id.is_empty() {
            self.get_lane_mut(from.unwrap()).unwrap().links.push(Link {
                from: from.unwrap(),
                via,
                to: to.unwrap(),
            });
            self.num_links += 1;
        } else {
            let trafficlight = self.get_trafficlight_id(trafficlight_id);
            if trafficlight.is_none() {
                return Err(format!(
                    "Link references an unknown traffic light {:?}",
                    trafficlight_id
                ));
            }
            let tls = self.get_trafficlight_mut(trafficlight.unwrap()).unwrap();
            while tls.links.len() <= link.link_index {
                tls.links.push(Vec::new());
            }
            tls.links[link.link_index].push(Link {
                from: from.unwrap(),
                via,
                to: to.unwrap(),
            });
            self.get_lane_mut(from.unwrap())
                .unwrap()
                .trafficlight
                .insert(trafficlight.unwrap());
            if let Some(v) = via {
                self.get_lane_mut(v)
                    .unwrap()
                    .trafficlight
                    .insert(trafficlight.unwrap());
            }
            self.get_lane_mut(to.unwrap())
                .unwrap()
                .trafficlight
                .insert(trafficlight.unwrap());
            self.num_controlled_links += 1;
        }
        Ok(())
    }
    fn insert_phase(&mut self, phase: &sumo::Phase) -> Result<(), String> {
        let trafficlight_id = unsafe { CStr::from_ptr(phase.trafficlight) };
        if trafficlight_id.is_empty() {
            return Err("Phase references an empty traffic light ID".to_string());
        }
        let trafficlight = self.get_trafficlight_id(trafficlight_id);
        if trafficlight.is_none() {
            return Err(format!(
                "Phase references an unknown traffic light {:?}",
                trafficlight_id
            ));
        }
        let tls = self.get_trafficlight_mut(trafficlight.unwrap()).unwrap();
        while tls.phases.len() <= phase.program_index {
            tls.phases.push(Vec::new());
        }
        while tls.phases[phase.program_index].len() <= phase.phase_index {
            tls.phases[phase.program_index].push(Phase {
                sigs: Signals::empty(),
                next: Vec::new(),
            });
        }
        if (!tls.phases[phase.program_index][phase.phase_index]
            .sigs
            .is_empty())
            && (!tls.phases[phase.program_index][phase.phase_index]
                .next
                .is_empty())
        {
            return Err(format!(
                "Duplicate phase: traffic_light={:?}, program_index={}, phase_index={}",
                trafficlight_id, phase.program_index, phase.phase_index
            ));
        }
        let state: String = unsafe { CStr::from_ptr(phase.state) }
            .to_string_lossy()
            .into();
        let mut next = Vec::new();
        if phase.next_count > 0 {
            next.extend_from_slice(unsafe {
                std::slice::from_raw_parts(phase.next_indexs, phase.next_count)
            });
        }
        let sigs = match Signals::from_str(&state) {
            Ok(s) => s,
            Err(e) => {
                return Err(format!("Invalid traffic signal state in phase: {}", e));
            }
        };
        tls.phases[phase.program_index][phase.phase_index].sigs = sigs;
        tls.phases[phase.program_index][phase.phase_index].next = next;
        self.num_phases += 1;
        Ok(())
    }
    fn check(&self) -> Result<(), String> {
        for tls in &self.trafficlights {
            for program in &tls.phases {
                for phase in program {
                    if phase.sigs.is_empty() && phase.next.is_empty() {
                        return Err(
                            "Road network contains an empty traffic signal phase".to_string()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_num_edges(&self) -> usize {
        self.edge_ids.len()
    }
    pub fn get_num_junctions(&self) -> usize {
        self.junction_ids.len()
    }
    pub fn get_num_lanes(&self) -> usize {
        self.lane_ids.len()
    }
    pub fn get_num_trafficlights(&self) -> usize {
        self.trafficlight_ids.len()
    }
    pub fn get_num_links(&self) -> (usize, usize) {
        (self.num_links, self.num_controlled_links)
    }
    pub fn get_num_phases(&self) -> usize {
        self.num_phases
    }
    pub fn get_edge_id(&self, edge: &CStr) -> Option<EdgeID> {
        self.edge_ids.get(edge).copied()
    }
    pub fn get_junction_id(&self, junction: &CStr) -> Option<JunctionID> {
        self.junction_ids.get(junction).copied()
    }
    pub fn get_lane_id(&self, lane: &CStr) -> Option<LaneID> {
        self.lane_ids.get(lane).copied()
    }
    pub fn get_trafficlight_id(&self, trafficlight: &CStr) -> Option<TrafficLightID> {
        self.trafficlight_ids.get(trafficlight).copied()
    }
    pub fn get_trafficlight(&self, trafficlight_id: TrafficLightID) -> Option<&TrafficLight> {
        if self.trafficlights.len() > trafficlight_id.to_usize() {
            Some(&self.trafficlights[trafficlight_id.to_usize()])
        } else {
            None
        }
    }
    pub fn get_lane(&self, lane_id: LaneID) -> Option<&Lane> {
        if self.lanes.len() > lane_id.to_usize() {
            Some(&self.lanes[lane_id.to_usize()])
        } else {
            None
        }
    }
    pub fn get_edge(&self, edge_id: EdgeID) -> Option<&Edge> {
        if self.edges.len() > edge_id.to_usize() {
            Some(&self.edges[edge_id.to_usize()])
        } else {
            None
        }
    }
    pub fn get_junction(&self, junction_id: JunctionID) -> Option<&Junction> {
        if self.junctions.len() > junction_id.to_usize() {
            Some(&self.junctions[junction_id.to_usize()])
        } else {
            None
        }
    }
    fn get_trafficlight_mut(
        &mut self,
        trafficlight_id: TrafficLightID,
    ) -> Option<&mut TrafficLight> {
        if self.trafficlights.len() > trafficlight_id.to_usize() {
            Some(&mut self.trafficlights[trafficlight_id.to_usize()])
        } else {
            None
        }
    }
    fn get_lane_mut(&mut self, lane_id: LaneID) -> Option<&mut Lane> {
        if self.lanes.len() > lane_id.to_usize() {
            Some(&mut self.lanes[lane_id.to_usize()])
        } else {
            None
        }
    }
    #[allow(unused)]
    fn get_edge_mut(&mut self, edge_id: EdgeID) -> Option<&mut Edge> {
        if self.edges.len() > edge_id.to_usize() {
            Some(&mut self.edges[edge_id.to_usize()])
        } else {
            None
        }
    }
    #[allow(unused)]
    fn get_junction_mut(&mut self, junction_id: JunctionID) -> Option<&mut Junction> {
        if self.junctions.len() > junction_id.to_usize() {
            Some(&mut self.junctions[junction_id.to_usize()])
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EdgeID(usize);

impl EdgeID {
    fn from_usize(value: usize) -> Self {
        Self(value)
    }

    fn to_usize(self) -> usize {
        self.0
    }
}

impl<'py> IntoPyObject<'py> for EdgeID {
    type Target = pyo3::types::PyInt;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_pyobject(py)
    }
}
impl FromPyObject<'_, '_> for EdgeID {
    type Error = PyErr;

    fn extract(
        obj: pyo3::prelude::Borrowed<'_, '_, pyo3::prelude::PyAny>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(obj.extract::<usize>()?))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct JunctionID(usize);

impl JunctionID {
    fn from_usize(value: usize) -> Self {
        Self(value)
    }

    fn to_usize(self) -> usize {
        self.0
    }
}

impl<'py> IntoPyObject<'py> for JunctionID {
    type Target = pyo3::types::PyInt;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_pyobject(py)
    }
}
impl FromPyObject<'_, '_> for JunctionID {
    type Error = PyErr;

    fn extract(
        obj: pyo3::prelude::Borrowed<'_, '_, pyo3::prelude::PyAny>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(obj.extract::<usize>()?))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LaneID(usize);

impl LaneID {
    fn from_usize(value: usize) -> Self {
        Self(value)
    }

    fn to_usize(self) -> usize {
        self.0
    }
}
impl<'py> IntoPyObject<'py> for LaneID {
    type Target = pyo3::types::PyInt;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_pyobject(py)
    }
}
impl FromPyObject<'_, '_> for LaneID {
    type Error = PyErr;

    fn extract(
        obj: pyo3::prelude::Borrowed<'_, '_, pyo3::prelude::PyAny>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(obj.extract::<usize>()?))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TrafficLightID(usize);

impl TrafficLightID {
    pub fn from_usize(value: usize) -> Self {
        Self(value)
    }

    pub fn to_usize(self) -> usize {
        self.0
    }
}

impl<'py> IntoPyObject<'py> for TrafficLightID {
    type Target = pyo3::types::PyInt;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_pyobject(py)
    }
}
impl FromPyObject<'_, '_> for TrafficLightID {
    type Error = PyErr;

    fn extract(
        obj: pyo3::prelude::Borrowed<'_, '_, pyo3::prelude::PyAny>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(obj.extract::<usize>()?))
    }
}

#[derive(Clone)]
#[pyclass(frozen, skip_from_py_object)]
pub struct Edge {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub from: Option<JunctionID>,
    #[pyo3(get)]
    pub to: Option<JunctionID>,
    #[pyo3(get)]
    pub lane_number: usize,
}

#[derive(Clone)]
#[pyclass(frozen, skip_from_py_object)]
pub struct Junction {
    #[pyo3(get)]
    pub id: String,
}

#[derive(Clone)]
#[pyclass(frozen, skip_from_py_object)]
pub struct Lane {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub edge_id: Option<EdgeID>,
    #[pyo3(get)]
    pub length: f64,
    #[pyo3(get)]
    pub links: Vec<Link>,
    #[pyo3(get)]
    pub trafficlight: HashSet<TrafficLightID>,
}

#[derive(Clone)]
#[pyclass(frozen, skip_from_py_object)]
pub struct TrafficLight {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub links: Vec<Vec<Link>>,
    #[pyo3(get)]
    pub phases: Vec<Vec<Phase>>,
}

#[pyclass(frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct Link {
    #[pyo3(get)]
    pub from: LaneID,
    #[pyo3(get)]
    pub via: Option<LaneID>,
    #[pyo3(get)]
    pub to: LaneID,
}

#[pyclass(frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct Phase {
    #[pyo3(get)]
    pub sigs: Signals,
    #[pyo3(get)]
    pub next: Vec<usize>,
}

#[derive(Clone)]
#[allow(non_camel_case_types)]
enum Signal {
    r,
    u,
    G,
    g,
    Y,
    y,
    o,
    O,
}
#[derive(Clone)]
pub struct Signals {
    inner: Vec<Signal>,
}

impl Signals {
    pub fn from_str(str: &str) -> Result<Self, String> {
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
                    return Err(format!("Invalid traffic signal character: {}", e));
                }
            }
        }
        Ok(Self { inner: vec })
    }
    pub fn to_str(&self) -> String {
        let mut str = String::new();
        for sig in &self.inner {
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
    pub fn empty() -> Self {
        Self { inner: Vec::new() }
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<'py> IntoPyObject<'py> for Signals {
    type Target = pyo3::types::PyString;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: pyo3::prelude::Python<'py>) -> Result<Self::Output, Self::Error> {
        self.to_str().into_pyobject(py)
    }
}

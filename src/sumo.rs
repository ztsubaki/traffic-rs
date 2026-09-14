use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_double, c_void};
use std::marker::PhantomData;
use std::path::Path;

pub struct Sumo {
    _not_thread_sync: PhantomData<Cell<()>>,
}

impl Drop for Sumo {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            log::warn!("SUMO reported an error while closing: {}", e);
        }
    }
}

#[repr(C)]
struct Payload {
    status: usize,
    payload: *mut c_void,
    len: usize,
}
#[repr(C)]
pub struct SimulationStatistics {
    pub current_time_millis: usize,
    pub active_vehicle_count: usize,
    pub loaded_vehicle_count: usize,
    pub departed_vehicle_count: usize,
    pub arrived_vehicle_count: usize,
    pub starting_teleport_count: usize,
    pub ending_teleport_count: usize,
    pub collision_count: usize,
    pub min_expected_vehicle_count: usize,
}
#[repr(C)]
#[derive(Clone)]
pub struct LaneTrafficMetrics {
    pub vehicle_count: usize,
    pub queued_vehicle_count: usize,
    pub mean_speed: f64,
    pub occupancy_percent: f64,
    pub waiting_time_seconds: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Position {
    pub x: c_double,
    pub y: c_double,
}
#[repr(C)]
pub struct Shape {
    pub points: *mut Position,
    pub len: usize,
}
#[repr(C)]
pub struct Vehicle {
    pub id: *const c_char,
    pub lane_id: *const c_char,
    pub lane_position: c_double,
    pub speed: c_double,
    pub acceleration: c_double,
    pub waiting_time: c_double,
    pub time_loss: c_double,
    pub next_trafficlight: *const c_char,
    pub next_link_index: usize,
    pub position: Position,
}

impl Drop for Vehicle {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            libc::free(self.lane_id as *mut c_void);
            libc::free(self.next_trafficlight as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Edge {
    pub id: *const c_char,
    pub name: *const c_char,
    pub from: *const c_char,
    pub to: *const c_char,
    pub lane_number: usize,
}

impl Drop for Edge {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            libc::free(self.name as *mut c_void);
            libc::free(self.from as *mut c_void);
            libc::free(self.to as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Junction {
    pub id: *const c_char,
    pub shape: Shape,
}

impl Drop for Junction {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            libc::free(self.shape.points as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Lane {
    pub id: *const c_char,
    pub edge_id: *const c_char,
    pub length: f64,
    pub shape: Shape,
}

impl Drop for Lane {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            libc::free(self.edge_id as *mut c_void);
            libc::free(self.shape.points as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct TrafficLight {
    pub id: *const c_char,
}

impl Drop for TrafficLight {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Link {
    pub from_lane: *const c_char,
    pub via_lane: *const c_char,
    pub to_lane: *const c_char,
    pub trafficlight: *const c_char,
    pub link_index: usize,
}

impl Drop for Link {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.from_lane as *mut c_void);
            libc::free(self.via_lane as *mut c_void);
            libc::free(self.to_lane as *mut c_void);
            libc::free(self.trafficlight as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Phase {
    pub trafficlight: *const c_char,
    pub state: *const c_char,
    pub program_index: usize,
    pub phase_index: usize,
    pub next_indexs: *const usize,
    pub next_count: usize,
}

impl Drop for Phase {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.trafficlight as *mut c_void);
            libc::free(self.state as *mut c_void);
            libc::free(self.next_indexs as *mut c_void);
        }
    }
}

pub struct CVec<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> CVec<T> {
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    #[allow(unused)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0 {
            return &mut [];
        }
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T> Drop for CVec<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            for index in 0..self.len {
                std::ptr::drop_in_place(self.ptr.add(index));
            }
            libc::free(self.ptr as *mut c_void);
        }
    }
}

unsafe extern "C" {
    fn sumo_load(
        cfg: *const c_char,
        threads: usize,
        statics: *const c_char,
        tripinfo: *const c_char,
        edgedata: *const c_char,
    ) -> Payload;
    fn sumo_step() -> Payload;
    fn sumo_close() -> Payload;
    fn get_parameter(obj: *const c_char, para: *const c_char) -> Payload;
    fn get_option(opt: *const c_char) -> Payload;
    fn set_trafficlight(trafficlight_id: *const c_char, state: *const c_char) -> Payload;
    fn get_trafficlight_state(trafficlight_id: *const c_char) -> Payload;
    fn get_lane_traffic_metrics(lane_id: *const c_char) -> Payload;
    fn get_edges() -> Payload;
    fn get_vehicles() -> Payload;
    fn get_junctions() -> Payload;
    fn get_lanes() -> Payload;
    fn get_trafficlights() -> Payload;
    fn get_links() -> Payload;
    fn get_phases() -> Payload;
    fn get_simulation_statistics() -> Payload;
}

const BRIDGE_SUCCESS: usize = 0;
const BRIDGE_SUMO_EXCEPTION: usize = 1;
const BRIDGE_MALLOC_FAILED: usize = 2;

impl Sumo {
    pub fn load(
        sumo_config_path: &Path,
        thread_count: usize,
        output_directory: &Path,
    ) -> Result<Sumo, String> {
        if thread_count < 1 {
            return Err("SUMO thread count must be at least 1".to_string());
        }
        let statics = output_directory.join("statics.xml");
        let tripinfo = output_directory.join("tripinfo.xml");
        let edgedata = output_directory.join("edgedata.xml");
        let sumo_config = path_to_c_string(sumo_config_path, "SUMO configuration path")?;
        let statics_path = path_to_c_string(&statics, "SUMO statistics output path")?;
        let tripinfo_path = path_to_c_string(&tripinfo, "SUMO trip information output path")?;
        let edgedata_path = path_to_c_string(&edgedata, "SUMO edge data output path")?;
        match resolve_c_result_no_payload(unsafe {
            sumo_load(
                sumo_config.as_ptr(),
                thread_count,
                statics_path.as_ptr(),
                tripinfo_path.as_ptr(),
                edgedata_path.as_ptr(),
            )
        }) {
            Err(e) => Err(e),
            Ok(()) => Ok(Sumo {
                _not_thread_sync: PhantomData,
            }),
        }
    }
    pub fn step_length(&self) -> Result<usize, String> {
        self.get_option("step-length")?
            .parse::<f64>()
            .map_or(Err("Failed to parse SUMO step length".to_string()), |f| {
                Ok((f * 1000.0) as usize)
            })
    }
    pub fn time_begin(&self) -> Result<usize, String> {
        self.get_option("begin")?
            .parse::<f64>()
            .map_or(Err("Failed to parse SUMO start time".to_string()), |f| {
                Ok((f * 1000.0) as usize)
            })
    }
    pub fn time_end(&self) -> Result<usize, String> {
        self.get_option("end")?.parse::<f64>().map_or(
            Err("Failed to parse SUMO end time".to_string()),
            |f| {
                Ok(if f < 0.0 {
                    usize::MAX
                } else {
                    (f * 1000.0) as usize
                })
            },
        )
    }
    pub fn net_file(&self) -> Result<String, String> {
        self.get_option("net-file")
    }
    pub fn route_files(&self) -> Result<String, String> {
        self.get_option("route-files")
    }
    pub fn step(&self) -> Result<(), String> {
        resolve_c_result_no_payload(unsafe { sumo_step() })
    }
    fn close(&mut self) -> Result<(), String> {
        resolve_c_result_no_payload(unsafe { sumo_close() })
    }
    pub fn get_edges(&self) -> Result<CVec<Edge>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_edges() })
    }
    pub fn get_vehicles(&self) -> Result<CVec<Vehicle>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_vehicles() })
    }
    pub fn get_junctions(&self) -> Result<CVec<Junction>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_junctions() })
    }
    pub fn get_trafficlights(&self) -> Result<CVec<TrafficLight>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_trafficlights() })
    }
    pub fn get_lanes(&self) -> Result<CVec<Lane>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_lanes() })
    }
    pub fn get_links(&self) -> Result<CVec<Link>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_links() })
    }
    pub fn get_phases(&self) -> Result<CVec<Phase>, String> {
        resolve_c_result_with_vec_payload(unsafe { get_phases() })
    }
    pub fn get_simulation_statistics(&self) -> Result<SimulationStatistics, String> {
        resolve_c_result_with_struct_payload(unsafe { get_simulation_statistics() })
    }
    #[allow(unused)]
    fn get_parameter(&self, object: &str, parameter: &str) -> Result<String, String> {
        let object = to_c_string(object, "SUMO parameter object")?;
        let parameter = to_c_string(parameter, "SUMO parameter name")?;
        match resolve_c_result_with_payload(unsafe {
            get_parameter(object.as_ptr(), parameter.as_ptr())
        }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload);
                }
                if r.is_empty() {
                    Err("SUMO parameter was not found".to_string())
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }
    fn get_option(&self, option: &str) -> Result<String, String> {
        let option = to_c_string(option, "SUMO option name")?;
        match resolve_c_result_with_payload(unsafe { get_option(option.as_ptr()) }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload);
                }
                if r.is_empty() {
                    Err("SUMO option was not found".to_string())
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }
    pub fn set_trafficlight(&self, trafficlight_id: &str, state: &str) -> Result<(), String> {
        let trafficlight_id = to_c_string(trafficlight_id, "traffic light ID")?;
        let state = to_c_string(state, "traffic light state")?;
        resolve_c_result_no_payload(unsafe {
            set_trafficlight(trafficlight_id.as_ptr(), state.as_ptr())
        })
    }
    pub fn get_trafficlight_state(&self, trafficlight_id: &str) -> Result<String, String> {
        let trafficlight_id = to_c_string(trafficlight_id, "traffic light ID")?;
        match resolve_c_result_with_payload(unsafe {
            get_trafficlight_state(trafficlight_id.as_ptr())
        }) {
            Ok(payload) => {
                let state = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into_owned();
                unsafe {
                    libc::free(payload);
                }
                Ok(state)
            }
            Err(error) => Err(error),
        }
    }
    pub fn get_lane_traffic_metrics(&self, lane_id: &str) -> Result<LaneTrafficMetrics, String> {
        let lane_id = to_c_string(lane_id, "lane ID")?;
        resolve_c_result_with_struct_payload(unsafe { get_lane_traffic_metrics(lane_id.as_ptr()) })
    }
}

fn path_to_c_string(path: &Path, description: &str) -> Result<CString, String> {
    let path = path
        .to_str()
        .ok_or_else(|| format!("{description} is not valid UTF-8: {}", path.display()))?;
    to_c_string(path, description)
}

fn to_c_string(value: &str, description: &str) -> Result<CString, String> {
    CString::new(value)
        .map_err(|error| format!("{description} contains an embedded null byte: {error}"))
}

fn resolve_c_result_no_payload(value: Payload) -> Result<(), String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(()),
        BRIDGE_MALLOC_FAILED => Err("SUMO bridge memory allocation failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("SUMO bridge returned an unknown status".to_string()),
    }
}

fn resolve_c_result_with_payload(value: Payload) -> Result<*mut c_void, String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(value.payload),
        BRIDGE_MALLOC_FAILED => Err("SUMO bridge memory allocation failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("SUMO bridge returned an unknown status".to_string()),
    }
}

fn resolve_c_result_with_vec_payload<T>(value: Payload) -> Result<CVec<T>, String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(CVec {
            ptr: value.payload as *mut T,
            len: value.len,
        }),
        BRIDGE_MALLOC_FAILED => Err("SUMO bridge memory allocation failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("SUMO bridge returned an unknown status".to_string()),
    }
}

fn resolve_c_result_with_struct_payload<T>(value: Payload) -> Result<T, String> {
    let payload = resolve_c_result_with_payload(value)?;
    let result = unsafe { (payload as *const T).read() };
    unsafe { libc::free(payload) };
    Ok(result)
}

use std::ffi::{CStr, CString, c_char, c_double, c_void};
use std::fs;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;

pub struct Sumo {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[repr(C)]
struct Payload {
    status: usize,
    payload: *mut c_void,
    len: usize,
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
}

impl Drop for Junction {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
        }
    }
}

#[repr(C)]
pub struct Lane {
    pub id: *const c_char,
    pub edge_id: *const c_char,
    pub length: f64,
}

impl Drop for Lane {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            libc::free(self.edge_id as *mut c_void);
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
    fn sumo_load(cfg: *const c_char, threads: usize) -> Payload;
    fn sumo_step() -> Payload;
    fn sumo_close() -> Payload;
    fn get_parameter(obj: *const c_char, para: *const c_char) -> Payload;
    fn get_option(opt: *const c_char) -> Payload;
    fn get_edges() -> Payload;
    fn get_vehicles() -> Payload;
    fn get_junctions() -> Payload;
    fn get_lanes() -> Payload;
    fn get_trafficlights() -> Payload;
    fn get_links() -> Payload;
    fn get_phases() -> Payload;
}

const BRIDGE_SUCCESS: usize = 0;
const BRIDGE_SUMO_EXCEPTION: usize = 1;
const BRIDGE_MALLOC_FAILED: usize = 2;

impl Sumo {
    pub fn load(sumocfg: &PathBuf, thread: usize) -> Result<Sumo, String> {
        if thread < 1 {
            return Err("Sumo threads must no less than 1".to_string());
        }
        let cfg_metadata = match fs::metadata(sumocfg) {
            Ok(metadata) => metadata,
            Err(e) => return Err(e.to_string()),
        };
        if !cfg_metadata.is_file() {
            return Err("Sumocfg is not a regular file".to_string());
        }
        match resolve_c_result_no_paylod(unsafe {
            sumo_load(
                CString::from_str(sumocfg.to_str().unwrap())
                    .unwrap()
                    .as_ptr(),
                thread,
            )
        }) {
            Err(e) => Err(e),
            Ok(()) => Ok(Sumo {
                _not_thread_safe: PhantomData,
            }),
        }
    }
    pub fn step_length(&self) -> Result<usize, String> {
        self.get_option("step-length")?
            .parse::<f64>()
            .map_or(
                Err("Parse error".to_string()),
                |f| Ok((f * 1000.0) as usize),
            )
    }
    pub fn time_begin(&self) -> Result<usize, String> {
        self.get_option("begin")?
            .parse::<f64>()
            .map_or(
                Err("Parse error".to_string()),
                |f| Ok((f * 1000.0) as usize),
            )
    }
    pub fn time_end(&self) -> Result<usize, String> {
        self.get_option("end")?
            .parse::<f64>()
            .map_or(
                Err("Parse error".to_string()),
                |f| Ok((f * 1000.0) as usize),
            )
    }
    pub fn net_file(&self) -> Result<String, String> {
        self.get_option("net-file")
    }
    pub fn route_files(&self) -> Result<String, String> {
        self.get_option("route-files")
    }
    pub fn step(&self) -> Result<(), String> {
        resolve_c_result_no_paylod(unsafe { sumo_step() })
    }
    pub fn close(self) -> Result<(), String> {
        resolve_c_result_no_paylod(unsafe { sumo_close() })
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
    #[allow(unused)]
    fn get_parameter(&self, obj: &str, para: &str) -> Result<String, String> {
        match resolve_c_result_with_payload(unsafe {
            get_parameter(
                CString::new(obj).unwrap().as_ptr(),
                CString::new(para).unwrap().as_ptr(),
            )
        }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload);
                }
                if r.is_empty() {
                    Err("No such key".to_string())
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }
    fn get_option(&self, opt: &str) -> Result<String, String> {
        match resolve_c_result_with_payload(unsafe {
            get_option(CString::new(opt).unwrap().as_ptr())
        }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload);
                }
                if r.is_empty() {
                    Err("No such key".to_string())
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }
}

fn resolve_c_result_no_paylod(value: Payload) -> Result<(), String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(()),
        BRIDGE_MALLOC_FAILED => Err("Bridge malloc failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("Unknow bridge status".to_string()),
    }
}

fn resolve_c_result_with_payload(value: Payload) -> Result<*mut c_void, String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(value.payload),
        BRIDGE_MALLOC_FAILED => Err("Bridge malloc failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("Unknow bridge status".to_string()),
    }
}

fn resolve_c_result_with_vec_payload<T>(value: Payload) -> Result<CVec<T>, String> {
    match value.status {
        BRIDGE_SUCCESS => Ok(CVec {
            ptr: value.payload as *mut T,
            len: value.len,
        }),
        BRIDGE_MALLOC_FAILED => Err("Bridge malloc failed".to_string()),
        BRIDGE_SUMO_EXCEPTION => {
            let mut e: String = unsafe { CStr::from_ptr(value.payload as *const i8) }
                .to_string_lossy()
                .into();
            unsafe { libc::free(value.payload) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("Unknow bridge status".to_string()),
    }
}

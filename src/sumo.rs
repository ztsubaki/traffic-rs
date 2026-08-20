use std::ffi::{CStr, CString, c_char, c_void};
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
    pub edge_id: *const c_char,
    pub lane_index: usize,
}

impl Drop for Vehicle {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
            if !self.edge_id.is_null() {
                libc::free(self.edge_id as *mut c_void);
            }
        }
    }
}

#[repr(C)]
pub struct Edge {
    pub id: *const c_char,
}

impl Drop for Edge {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.id as *mut c_void);
        }
    }
}

pub struct CVec<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> CVec<T> {
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0{
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0{
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
                std::ptr::drop_in_place((self.ptr as *mut T).add(index));
            }
            libc::free(self.ptr as *mut c_void);
        }
    }
}

unsafe extern "C" {
    fn sumo_load(cfg: *const c_char) -> Payload;
    fn sumo_step() -> Payload;
    fn sumo_close() -> Payload;
    fn get_parameter(obj: *const c_char, para: *const c_char) -> Payload;
    fn get_option(opt: *const c_char) -> Payload;
    fn get_edges() -> Payload;
    fn get_vehicles() -> Payload;
}

const BRIDGE_SUCCESS: usize = 0;
const BRIDGE_SUMO_EXCEPTION: usize = 1;
const BRIDGE_MALLOC_FAILED: usize = 2;

impl Sumo {
    pub fn load(sumocfg: &PathBuf) -> Result<Sumo, String> {
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
                    .as_ptr() as *const i8,
            )
        }) {
            Err(e) => Err(e),
            Ok(()) => Ok(Sumo {_not_thread_safe:PhantomData}),
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
    fn get_parameter(&self, obj: &str, para: &str) -> Result<String, String> {
        match resolve_c_result_with_payload(unsafe {
            get_parameter(
                CString::new(obj).unwrap().as_ptr() as *const i8,
                CString::new(para).unwrap().as_ptr() as *const i8,
            )
        }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload as *mut c_void);
                }
                if r.len() == 0 {
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
            get_option(CString::new(opt).unwrap().as_ptr() as *const i8)
        }) {
            Ok(payload) => {
                let r: String = unsafe { CStr::from_ptr(payload as *const i8) }
                    .to_string_lossy()
                    .into();
                unsafe {
                    libc::free(payload as *mut c_void);
                }
                if r.len() == 0 {
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
            unsafe { libc::free(value.payload as *mut c_void) };
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
            unsafe { libc::free(value.payload as *mut c_void) };
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
            unsafe { libc::free(value.payload as *mut c_void) };
            e.insert_str(0, "SUMO exception: ");
            Err(e)
        }
        _ => Err("Unknow bridge status".to_string()),
    }
}

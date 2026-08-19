use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Sumo {}

#[repr(C)]
struct payload {
    status: usize,
    payload: usize,
}

unsafe extern "C" {
    fn load(cfg: *const c_char) -> usize;
    fn step() -> usize;
    fn close() -> usize;
    fn get_parameter(obj: *const c_char, para: *const c_char) -> payload;
    fn get_option(opt: *const c_char) -> payload;
    fn vehicle_get_id_list() -> payload;
}

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
            load(sumocfg.to_str().unwrap().as_ptr() as *const i8)
        }) {
            Err(e) => Err(e),
            Ok(()) => Ok(Sumo {}),
        }
    }
    pub fn step_length(&self) -> Result<usize, String> {
        self.get_option("step-length")?
            .parse::<f64>()
            .map_or(Err(String::from_str("Parse error").unwrap()), |f| {
                Ok((f * 1000.0) as usize)
            })
    }
    pub fn time_begin(&self) -> Result<usize, String> {
        self.get_option("begin")?
            .parse::<f64>()
            .map_or(Err(String::from_str("Parse error").unwrap()), |f| {
                Ok((f * 1000.0) as usize)
            })
    }
    pub fn time_end(&self) -> Result<usize, String> {
        self.get_option("end")?
            .parse::<f64>()
            .map_or(Err(String::from_str("Parse error").unwrap()), |f| {
                Ok((f * 1000.0) as usize)
            })
    }
    pub fn net_file(&self) -> Result<String, String> {
        self.get_option("net-file")
    }
    pub fn route_files(&self) -> Result<String, String> {
        self.get_option("route-files")
    }
    pub fn step(&self) -> Result<(), String> {
        resolve_c_result_no_paylod(unsafe { step() })
    }
    pub fn close(self) -> Result<(), String> {
        resolve_c_result_no_paylod(unsafe { close() })
    }
    pub fn vechine_get_id_list(&self) -> Result<Vec<String>, String> {
        resolve_c_result_with_vec_payload(unsafe { vehicle_get_id_list() }, |addr| unsafe {
            CStr::from_ptr(addr as *const i8).to_string_lossy().into()
        })
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
                    Err(String::from_str("No such key").unwrap())
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
                    Err(String::from_str("No such key").unwrap())
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }
}

fn resolve_c_result_no_paylod(value: usize) -> Result<(), String> {
    if value == 0 {
        Ok(())
    } else if value == 1 {
        Err(String::from_str("<CPP RUNTIME INTERNAL ERROR>").unwrap())
    } else {
        let e = Err(unsafe { CStr::from_ptr(value as *const i8) }
            .to_string_lossy()
            .into());
        unsafe { libc::free(value as *mut c_void) }
        e
    }
}

fn resolve_c_result_with_payload(value: payload) -> Result<usize, String> {
    if value.status == 0 {
        Ok(value.payload)
    } else if value.status == 1 {
        Err(String::from_str("<CPP RUNTIME INTERNAL ERROR>").unwrap())
    } else {
        let e = Err(unsafe { CStr::from_ptr(value.status as *const i8) }
            .to_string_lossy()
            .into());
        unsafe { libc::free(value.status as *mut c_void) }
        e
    }
}

fn resolve_c_result_with_vec_payload<F, T>(value: payload, f: F) -> Result<Vec<T>, String>
where
    F: Fn(usize) -> T,
{
    if value.status == 0 {
        let mut vec = Vec::new();
        let mut ptr = value.payload as *mut usize;
        while unsafe { *ptr } != 0 {
            vec.push(f(unsafe { *ptr }));
            ptr = unsafe { ptr.add(1) };
        }
        Ok(vec)
    } else if value.status == 1 {
        Err(String::from_str("<CPP RUNTIME INTERNAL ERROR>").unwrap())
    } else {
        let e = Err(unsafe { CStr::from_ptr(value.status as *const i8) }
            .to_string_lossy()
            .into());
        unsafe { libc::free(value.status as *mut c_void) }
        e
    }
}

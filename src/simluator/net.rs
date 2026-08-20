use std::{collections::HashMap, ffi::CString};

pub struct Net {
    pub edges: HashMap<CString, usize>,
}

impl Net {
    pub fn new() -> Net {
        Net {
            edges: HashMap::new(),
        }
    }
}

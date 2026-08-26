use crate::{
    Sumo,
    simluator::net::{LaneID, Net, TrafficLightID},
    sumo,
};
use log::trace;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
};

pub struct RSU {}

impl RSU {
    pub fn new() -> RSU {
        RSU {}
    }
}

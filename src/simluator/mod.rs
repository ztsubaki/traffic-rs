use std::str::FromStr;

use crate::Sumo;

pub struct Simluator {
    sumo: Sumo,
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
        Simluator {
            sumo,
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
        if let Err(e) = self.sumo.step() {
            return Err(e);
        }
        self.step_current += 1;
        Ok(())
    }

    pub fn close(self) -> Result<(), String> {
        self.sumo.close()
    }

    pub fn vehicle_list(&self) -> Result<Vec<String>, String> {
        self.sumo.vechine_get_id_list()
    }
}

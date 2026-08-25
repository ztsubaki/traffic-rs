mod conf;
mod simluator;
mod sumo;

use clap::Parser;
use conf::CONFIG;
use log::{debug, error, info};
use std::path::PathBuf;
use std::process::exit;
use std::time::Instant;
use sumo::Sumo;

use crate::simluator::Simluator;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(author = "ztsubaki")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "a traffic simulator based on libsumo and use for evaluate python traffic control algorithm", long_about = None)]
pub struct Args {
    #[arg(short, long, value_name = "CONFIG FILE")]
    pub config: Option<PathBuf>,
}

lazy_static::lazy_static! {
    pub static ref ARGS : Args = Args::parse();
}

fn main() {
    env_logger::init();
    debug!(
        "Starting with config:\n{}",
        toml::to_string(&*CONFIG).unwrap()
    );
    let sumocfg = &CONFIG.sumo.sumocfg;

    info!("Loading SUMO");

    let sumo = match Sumo::load(sumocfg) {
        Ok(sumo) => sumo,
        Err(e) => {
            error!("Init SUMO failed: {}", e);
            exit(1);
        }
    };
    info!("time_begin: {}ms", sumo.time_begin().unwrap());
    info!("time_end: {}ms", sumo.time_end().unwrap());
    info!("step_length: {}ms", sumo.step_length().unwrap());
    info!("net-file: {}", sumo.net_file().unwrap());
    info!("route-files: {}", sumo.route_files().unwrap());

    info!("Inited SUMO");
    let mut simluator = Simluator::init(sumo);
    info!("Inited Simluator");

    let global_start = Instant::now();
    while !simluator.is_end() {
        simluator.tick().unwrap();
        if simluator.step_current.is_multiple_of(CONFIG.log_interval) {
            let end = global_start.elapsed();
            let simluation_elapsed = simluator.step_current * simluator.step;
            let vehicles = simluator.vehicle_list().unwrap();
            info!("loaded {} vechines", vehicles.len());
            info!(
                "setp: {} simulation time: {}ms cost: {}ms procress: {:.2}% rate:{:.2}x",
                simluator.step_current,
                simluator.time_begin + simluation_elapsed,
                end.as_millis(),
                ((simluation_elapsed * 100) as f64)
                    / ((simluator.time_end - simluator.time_begin) as f64),
                (simluation_elapsed as f64) / (end.as_millis() as f64)
            );
        }
    }

    info!("Simluate ended");

    simluator.close().unwrap();
}

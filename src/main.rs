mod conf;
mod simluator;
mod sumo;

use clap::Parser;
use conf::CONFIG;
use log::{debug, error, info,warn};
use rand_distr::Normal;
use std::env;
use std::path::{Path, PathBuf};
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
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "info") }
    }
    env_logger::init();
    debug!(
        "Starting with config:\n{}",
        toml::to_string(&*CONFIG).unwrap()
    );

    lazy_static::initialize(&CONFIG);

    info!("Loading SUMO");

    let pwd = match env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            error!("Can not get pwd: {}", e);
            exit(1);
        }
    };
    let cfg_dir = pwd
        .join(match ARGS.config.as_ref().unwrap().parent() {
            Some(p) => p,
            None => {
                error!(
                    "Can not get parent for {}",
                    ARGS.config.clone().unwrap().display()
                );
                exit(1);
            }
        });

    let output_dir = cfg_dir.join(&CONFIG.output_dir);
    if output_dir.exists(){
        if !output_dir.is_dir(){
            error!("Output dir {} is not directory",output_dir.display());
            exit(1);
        }else{
            warn!("Output dir already exists, pervious output may be covered");
        }
    }else{
        match std::fs::create_dir(&output_dir) {
            Ok(_) => {}
            Err(e) => {
                error!("Can not create output dir {}: {}",&output_dir.display(),e);
                exit(1);
            }
        }
    }

    info!("Using output directory {}",output_dir.display());

    let sumocfg = cfg_dir.join(&CONFIG.sumo.sumocfg);

    match sumocfg.parent() {
        Some(path) => {
            if path.is_dir() {
                if let Err(e) = env::set_current_dir(path) {
                    error!("Can not change dir to {}: {}", path.display(), e);
                    exit(1);
                }
            } else {
                error!("Sumocfg dir {} is not directory", path.display());
                exit(1);
            }
        }
        None => {
            error!("Can not get parent for {}", sumocfg.display());
            exit(1);
        }
    }

    if !sumocfg.exists() || !sumocfg.is_file() {
        error!("Can not open sumocfg file {}", sumocfg.display());
        exit(1);
    }

    let sumo = match Sumo::load(&sumocfg, CONFIG.sumo.threads, &output_dir) {
        Ok(sumo) => sumo,
        Err(e) => {
            error!("Init SUMO failed: {}", e);
            exit(1);
        }
    };

    if let Err(e) = env::set_current_dir(pwd.clone()) {
        error!("Can not change dir back to {}: {}", pwd.display(), e);
        exit(1);
    }

    info!("time_begin: {}ms", sumo.time_begin().unwrap());
    info!("time_end: {}ms", sumo.time_end().unwrap());
    info!("step_length: {}ms", sumo.step_length().unwrap());
    info!("net-file: {}", sumo.net_file().unwrap());
    info!("route-files: {}", sumo.route_files().unwrap());

    info!("Inited SUMO");
    let cloud_delay_generator = Normal::new(
        CONFIG.cloud_delay_mean as f32,
        CONFIG.cloud_delay_std_dev as f32,
    )
    .unwrap();
    let mut simluator = Simluator::init(sumo, cloud_delay_generator);
    info!("Inited Simluator");

    let global_start = Instant::now();
    while !simluator.is_end() {
        simluator.tick().unwrap();
        if simluator.step_current.is_multiple_of(CONFIG.log_interval) {
            let end = global_start.elapsed();
            let simluation_elapsed = simluator.step_current * simluator.step;
            info!(
                "setp: {} simulation time: {}ms cost: {}ms procress: {:.2}% rate:{:.2}x",
                simluator.step_current,
                simluator.time_begin + simluation_elapsed,
                end.as_millis(),
                ((simluation_elapsed * 100) as f64)
                    / ((simluator.time_end - simluator.time_begin) as f64),
                (simluation_elapsed as f64) / (end.as_millis() as f64)
            );
            let (delay, loss) = simluator.v2x_stats();
            info!(
                "V2X average delay: {:.2} ms, package loss rate {:.2}%",
                delay,
                loss * 100.0
            );
        }
    }
    info!("Simluate ended");
}

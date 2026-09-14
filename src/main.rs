mod api;
mod conf;
mod net;
mod simulator;
mod sumo;

use clap::Parser;
use conf::CONFIG;
use log::{debug, error, info, warn};
use pyo3::{Python, types::PyModule};
use std::ffi::CString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use std::{env, thread};

use simulator::{Simulator, trafficrs};
use sumo::Sumo;

use crate::net::Net;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(author = "ztsubaki")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "a traffic simulator based on libsumo and use for evaluate python traffic control algorithm", long_about = None)]
pub struct Args {
    /// config file path
    #[arg(short, long, value_name = "CONFIG FILE")]
    pub config: Option<PathBuf>,
    /// do not apply plugin, just use SUMO
    #[arg(long)]
    pub baseline: bool,
}

lazy_static::lazy_static! {
    pub static ref ARGS : Args = Args::parse();
}

fn main() -> ExitCode {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "info") }
    }
    env_logger::init();
    debug!(
        "Resolved configuration:\n{}",
        toml::to_string(&*CONFIG).unwrap()
    );

    lazy_static::initialize(&CONFIG);

    if !(CONFIG.cloud_packet_loss_rate >= 0.0 && CONFIG.cloud_packet_loss_rate < 1.0) {
        error!(
            "Invalid cloud packet loss rate: value={} must be in [0.0, 1.0)",
            CONFIG.cloud_packet_loss_rate
        );
        return ExitCode::FAILURE;
    }

    info!("Initializing SUMO");

    let working_directory = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            error!(
                "Failed to determine the current working directory: {}",
                error
            );
            return ExitCode::FAILURE;
        }
    };
    let config_path = match ARGS.config.as_ref() {
        Some(path) => path,
        None => {
            error!("No configuration file was specified.");
            return ExitCode::FAILURE;
        }
    };
    let config_directory = working_directory.join(match config_path.parent() {
        Some(p) => p,
        None => {
            error!(
                "Configuration path has no parent directory: {}",
                config_path.display()
            );
            return ExitCode::FAILURE;
        }
    });

    let output_dir = config_directory.join(&CONFIG.output_dir);
    if output_dir.exists() {
        if !output_dir.is_dir() {
            error!("Output path is not a directory: {}", output_dir.display());
            return ExitCode::FAILURE;
        } else {
            warn!(
                "Output directory already exists; previous simulation output may be overwritten: {}",
                output_dir.display()
            );
        }
    } else {
        match std::fs::create_dir_all(&output_dir) {
            Ok(_) => {}
            Err(e) => {
                error!(
                    "Failed to create output directory {}: {}",
                    output_dir.display(),
                    e
                );
                return ExitCode::FAILURE;
            }
        }
    }

    info!("Using output directory: {}", output_dir.display());

    let sumo_config_path = config_directory.join(&CONFIG.sumo.sumocfg);

    match sumo_config_path.parent() {
        Some(path) => {
            if path.is_dir() {
                if let Err(e) = env::set_current_dir(path) {
                    error!(
                        "Failed to change working directory to {}: {}",
                        path.display(),
                        e
                    );
                    return ExitCode::FAILURE;
                }
            } else {
                error!(
                    "SUMO configuration directory is not a directory: {}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        }
        None => {
            error!(
                "SUMO configuration path has no parent directory: {}",
                sumo_config_path.display()
            );
            return ExitCode::FAILURE;
        }
    }

    if !sumo_config_path.exists() || !sumo_config_path.is_file() {
        error!(
            "SUMO configuration file does not exist or is not a regular file: {}",
            sumo_config_path.display()
        );
        return ExitCode::FAILURE;
    }
    let sumo = match Sumo::load(&sumo_config_path, CONFIG.sumo.threads, &output_dir) {
        Ok(sumo) => sumo,
        Err(e) => {
            error!("Failed to initialize SUMO: {}", e);
            return ExitCode::FAILURE;
        }
    };
    let mut net = Net::new();
    if let Err(e) = net.init(&sumo) {
        error!("Failed to initialize road network: {}", e);
        return ExitCode::FAILURE;
    }

    if let Err(error) = env::set_current_dir(working_directory.clone()) {
        error!(
            "Failed to restore the working directory to {}: {}",
            working_directory.display(),
            error
        );
        return ExitCode::FAILURE;
    }

    info!(
        "SUMO initialized: start={}ms, end={}ms, step={}ms, net_file={}, route_files={}",
        sumo.time_begin().unwrap(),
        sumo.time_end().unwrap(),
        sumo.step_length().unwrap(),
        sumo.net_file().unwrap(),
        sumo.route_files().unwrap()
    );

    let simulation_net = Arc::new(net);
    let api_net = simulation_net.clone();
    let simulator = match if !ARGS.baseline {
        pyo3::append_to_inittab!(trafficrs);
        Python::initialize();

        match Python::attach(|py| {
            py.run(
                pyo3::ffi::c_str!(
                    r#"
import trafficrs

trafficrs.hello_world()
"#
                ),
                None,
                None,
            )
        }) {
            Ok(_) => {}
            Err(e) => {
                error!("Python binding self-test failed: {}", e);
                return ExitCode::FAILURE;
            }
        }

        info!("Python runtime initialized");
        let script = config_directory.join(&CONFIG.script);

        if !script.exists() {
            error!("Plugin script does not exist: {}", script.display());
            return ExitCode::FAILURE;
        }

        if !script.is_file() {
            error!("Plugin script is not a regular file: {}", script.display());
            return ExitCode::FAILURE;
        }

        let source = match std::fs::read_to_string(&script) {
            Ok(ctx) => ctx,
            Err(e) => {
                error!("Failed to read plugin script {}: {}", script.display(), e);
                return ExitCode::FAILURE;
            }
        };
        let source = match CString::new(source) {
            Ok(ctx) => ctx,
            Err(e) => {
                error!(
                    "Plugin script contains a null byte {}: {}",
                    script.display(),
                    e
                );
                return ExitCode::FAILURE;
            }
        };

        let module = match Python::attach(|py| {
            PyModule::from_code(
                py,
                &source,
                pyo3::ffi::c_str!("plugin.py"),
                pyo3::ffi::c_str!("plugin"),
            )
            .map(|m| m.unbind())
        }) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to load plugin script {}: {}", script.display(), e);
                return ExitCode::FAILURE;
            }
        };

        Simulator::init(sumo, Some(module), simulation_net)
    } else {
        info!("Running in baseline mode without the V2X plugin");
        Simulator::init(sumo, None, simulation_net)
    } {
        Ok(simulator) => simulator,
        Err(error) => {
            error!("Failed to initialize the simulation: {}", error);
            return ExitCode::FAILURE;
        }
    };

    info!("Simulation initialized");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|error| {
            error!("Failed to create the asynchronous runtime: {error}");
            ExitCode::FAILURE
        });
    let rt = match rt {
        Ok(runtime) => runtime,
        Err(exit_code) => return exit_code,
    };
    let simulation_simulator = Arc::new(Mutex::new(simulator));
    let api_simulator = simulation_simulator.clone();
    let (sim_to_api_tx, sim_to_api_rx) =
        tokio::sync::mpsc::channel::<Result<api::RunStat, String>>(1);
    let (api_to_sim_tx, api_to_sim_rx) = mpsc::channel::<usize>();
    let (tx0, rx0) = mpsc::channel();
    let tx1 = tx0.clone();
    thread::spawn(move || {
        if tx0
            .send(run_simulation(
                simulation_simulator,
                api_to_sim_rx,
                sim_to_api_tx,
            ))
            .is_err()
        {
            error!("Simulation worker exited after its result receiver was dropped.");
        }
    });
    thread::spawn(move || {
        if tx1
            .send(rt.block_on(api::run_api(
                api_simulator,
                api_net,
                api_to_sim_tx,
                sim_to_api_rx,
            )))
            .is_err()
        {
            error!("API server exited after its result receiver was dropped.");
        }
    });
    match rx0.recv() {
        Ok(exit_code) => exit_code,
        Err(error) => {
            error!("Both service workers exited without reporting a result: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_simulation(
    simulator: Arc<Mutex<Simulator>>,
    rx: Receiver<usize>,
    tx: tokio::sync::mpsc::Sender<Result<api::RunStat, String>>,
) -> ExitCode {
    loop {
        let requested_steps = match rx.recv() {
            Ok(steps) => steps,
            Err(_) => return ExitCode::SUCCESS,
        };
        let start = Instant::now();
        let mut simulator = match simulator.lock() {
            Ok(simulator) => simulator,
            Err(error) => {
                error!("Simulation state lock is poisoned: {error}");
                return ExitCode::FAILURE;
            }
        };
        let start_step = simulator.step_current;
        let mut simulate_duration = Duration::new(0, 0);
        let mut script_duration = Duration::new(0, 0);
        let mut sumo_duration = Duration::new(0, 0);
        let mut global_duration = Duration::new(0, 0);
        let mut ended = false;
        let mut run_error = None;
        for _ in 0..requested_steps {
            if simulator.is_end() {
                ended = true;
                break;
            }
            match simulator.tick() {
                Ok((duration0, duration1, duration2, duration3)) => {
                    simulate_duration += duration0;
                    script_duration += duration1;
                    sumo_duration += duration2;
                    global_duration += duration3;
                }
                Err(error) => {
                    let detail = format!(
                        "Simulation failed at step {} after completing {} step(s): {}",
                        simulator.step_current,
                        simulator.step_current - start_step,
                        error
                    );
                    error!("{detail}");
                    run_error = Some(detail);
                    break;
                }
            }
        }
        let time = start.elapsed();
        let end_step = simulator.step_current;
        info!(
            "Progress: step {}-{}, simulation_time {}ms, elapsed {}ms, avg_step_time={:.2}ms, complete={:.2}%, speed={:.2}x",
            start_step,
            end_step,
            (end_step - start_step) * simulator.step,
            time.as_millis(),
            if end_step > start_step {
                (time.as_millis() as f64) / ((end_step - start_step) as f64)
            } else {
                0.0
            },
            if (simulator.time_end - simulator.time_begin) > 0 {
                ((end_step * simulator.step * 100) as f64)
                    / ((simulator.time_end - simulator.time_begin) as f64)
            } else {
                100.0
            },
            if time.is_zero() {
                0.0
            } else {
                (((end_step - start_step) * simulator.step) as f64) / (time.as_millis() as f64)
            }
        );
        if !ARGS.baseline {
            let (delay, loss) = simulator.v2x_stats();
            info!(
                "V2X statistics: average_delay={:.2}ms, packet_loss={:.2}%",
                delay,
                loss * 100.0
            );
        }
        let result = match run_error {
            Some(detail) => Err(detail),
            None => Ok(api::RunStat {
                start_step,
                end_step,
                completed_steps: end_step - start_step,
                has_ended: ended,
                simulated_duration_millis: simulate_duration.as_millis() as usize,
                script_duration_millis: script_duration.as_millis() as usize,
                sumo_duration_millis: sumo_duration.as_millis() as usize,
                total_duration_millis: global_duration.as_millis() as usize,
            }),
        };
        if tx.try_send(result).is_err() {
            warn!("The API server is unavailable; discarding simulation run statistics.");
        }
    }
}

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;

use crate::ARGS;

lazy_static! {
    pub static ref CONFIG: Config = load_config();
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub log_interval: usize,
    pub rsu_report_interval: usize,
    pub rsu_bandwidth: usize,
    pub rsu_delay_min: usize,
    pub rsu_delay_k: f64,
    pub cloud_delay_mean: usize,
    pub cloud_delay_std_dev: usize,
    pub cloud_packet_loss_rate: f64,
    pub output_dir: PathBuf,
    pub script: PathBuf,
    pub listen_addr: IpAddr,
    pub listen_port: u16,
    pub sumo: Sumo,
}

impl Config {
    fn default() -> Config {
        Config {
            log_interval: 100,
            rsu_report_interval: 5000,
            rsu_bandwidth: 50,
            rsu_delay_min: 5,
            rsu_delay_k: 100.0,
            cloud_delay_mean: 20,
            cloud_delay_std_dev: 10,
            cloud_packet_loss_rate: 0.001,
            output_dir: PathBuf::from_str("output").unwrap(),
            script: PathBuf::from_str("plugin.py").unwrap(),
            listen_addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            listen_port: 8080,
            sumo: Sumo {
                sumocfg: PathBuf::new(),
                threads: 2,
            },
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Sumo {
    pub sumocfg: PathBuf,
    pub threads: usize,
}

fn load_config() -> Config {
    match &ARGS.config {
        Some(path) => {
            let file = match fs::File::open(path) {
                Ok(file) => file,
                Err(error) => {
                    log::error!(
                        "Failed to open configuration file {}: {}",
                        path.display(),
                        error
                    );
                    exit(1);
                }
            };
            let mut limited_file = file.take(10_241);
            let mut buffer = Vec::new();
            match limited_file.read_to_end(&mut buffer) {
                Ok(_) => {}
                Err(error) => {
                    log::error!(
                        "Failed to read configuration file {}: {}",
                        path.display(),
                        error
                    );
                    exit(1);
                }
            };
            if buffer.len() > 10_240 {
                log::error!(
                    "Configuration file exceeds the 10 KiB size limit: {}",
                    path.display()
                );
                exit(1);
            }
            match String::from_utf8(buffer) {
                Ok(str) => match toml::from_str(&str) {
                    Ok(config) => config,
                    Err(error) => {
                        log::error!(
                            "Failed to parse configuration file {}: {}",
                            path.display(),
                            error
                        );
                        exit(1);
                    }
                },
                Err(_) => {
                    log::error!("Configuration file is not valid UTF-8: {}", path.display());
                    exit(1);
                }
            }
        }
        None => {
            log::warn!("No configuration file was provided; creating a default configuration");
            let mut file = match fs::File::create_new("conf.toml") {
                Ok(f) => f,
                Err(error) => {
                    match error.kind() {
                        io::ErrorKind::AlreadyExists => {
                            log::error!(
                                "Default configuration file already exists; specify it with --config conf.toml"
                            );
                        }
                        _ => {
                            log::error!(
                                "Failed to create the default configuration file: {}",
                                error
                            );
                        }
                    };
                    exit(1);
                }
            };
            let default_config = match toml::to_string(&Config::default()) {
                Ok(config) => config,
                Err(error) => {
                    log::error!("Failed to serialize the default configuration: {}", error);
                    exit(1);
                }
            };
            match file.write_all(default_config.as_bytes()) {
                Ok(_) => {
                    exit(0);
                }
                Err(error) => {
                    log::error!("Failed to write the default configuration file: {}", error);
                    exit(1);
                }
            };
        }
    }
}

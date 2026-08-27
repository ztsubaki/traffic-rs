use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::{
    fs,
    io::{self, Read, Write},
    process::exit,
};

use crate::ARGS;

lazy_static! {
    pub static ref CONFIG: Config = get_conf();
}

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub log_interval: usize,
    pub rsu_report_interval: usize,
    pub rsu_bandwidth: usize,
    pub rsu_delay_min: usize,
    pub rsu_delay_k: f64,
    pub cloud_delay_mean: usize,
    pub cloud_delay_std_dev: usize,
    pub cloud_package_loss_rate: f64,
    pub output_dir: PathBuf,
    pub script: PathBuf,
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
            cloud_package_loss_rate: 0.001,
            output_dir: PathBuf::from_str("output").unwrap(),
            script: PathBuf::from_str("plugin.py").unwrap(),
            sumo: Sumo {
                sumocfg: PathBuf::new(),
                threads: 2,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Sumo {
    pub sumocfg: PathBuf,
    pub threads: usize,
}

fn get_conf() -> Config {
    match &ARGS.config {
        Some(path) => {
            let file = match fs::File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Connot open config file: {}", e);
                    exit(1);
                }
            };
            let mut file_limited = file.take(10241);
            let mut buffer = Vec::new();
            match file_limited.read_to_end(&mut buffer) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Can not read config: {}", e);
                    exit(1);
                }
            };
            if buffer.len() > 10240 {
                log::error!("Config file size more than 10KB, is it right?");
                exit(1);
            }
            match String::from_utf8(buffer) {
                Ok(str) => match toml::from_str(&str) {
                    Ok(config) => config,
                    Err(e) => {
                        log::error!("Prase config file error: {}", e);
                        exit(1);
                    }
                },
                Err(_) => {
                    log::error!("Config file is not UTF-8!");
                    exit(1);
                }
            }
        }
        None => {
            log::warn!("No config file provided, try to create default config");
            let mut file = match fs::File::create_new("conf.toml") {
                Ok(f) => f,
                Err(e) => {
                    match e.kind() {
                        io::ErrorKind::AlreadyExists => {
                            log::error!(
                                "Try to create default config but file already exists, did you forget \"-c conf.toml\" ?"
                            );
                        }
                        _ => {
                            log::error!("Can not create config file");
                        }
                    };
                    exit(1);
                }
            };
            match file.write_all(toml::to_string(&Config::default()).unwrap().as_bytes()) {
                Ok(_) => {
                    exit(0);
                }
                Err(e) => {
                    log::error!("Can not write to config: {}", e);
                    exit(1);
                }
            };
        }
    }
}

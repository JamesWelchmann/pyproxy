use std::env;
use std::error;
use std::net;
use std::path;
use std::str::FromStr;

pub struct Config {
    pub bind_addr: net::SocketAddr,
    pub output_addr: net::SocketAddr,
    pub rundir: path::PathBuf,
    pub num_workers: usize,
    pub workerbin: path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let bind = net::IpAddr::V4(net::Ipv4Addr::new(0, 0, 0, 0));

        let rundir = env::current_dir().expect("failed to get current working directory");

        let argv0 = std::env::args().next().expect("couldn't get argv0");

        let mut workerbin = path::PathBuf::from_str(&argv0).expect("argv0 not valid path");

        workerbin.pop();
        workerbin.push("worker");

        Self {
            bind_addr: net::SocketAddr::new(bind, 9000),
            output_addr: net::SocketAddr::new(bind, 9001),
            rundir,
            num_workers: 3,
            workerbin,
        }
    }
}

#[derive(Debug)]
pub struct EnvError {
    pub env_var: String,
    pub env_val: String,
    pub error: Box<dyn error::Error>,
}

#[derive(Debug)]
pub struct Error {
    pub errors: Vec<EnvError>,
}

pub fn from_env() -> Result<Config, Error> {
    let mut slf = Config::default();
    let mut errors = vec![];

    for (key, val) in env::vars() {
        match key.as_ref() {
            "PYPROXY_BIND_ADDR" => match val.parse() {
                Err(e) => {
                    errors.push(EnvError {
                        env_var: key.clone(),
                        env_val: val.clone(),
                        error: Box::new(e),
                    });
                }
                Ok(bind_addr) => {
                    slf.bind_addr = bind_addr;
                }
            },
            "PYPROXY_OUTPUT_ADDR" => match val.parse() {
                Err(e) => {
                    errors.push(EnvError {
                        env_var: key.clone(),
                        env_val: val.clone(),
                        error: Box::new(e),
                    });
                }
                Ok(output_addr) => {
                    slf.output_addr = output_addr;
                }
            },

            "PYPROXY_NUM_WORKERS" => match val.parse() {
                Err(e) => {
                    errors.push(EnvError {
                        env_var: key.clone(),
                        env_val: val.clone(),
                        error: Box::new(e),
                    });
                }
                Ok(num_workers) => {
                    slf.num_workers = num_workers;
                }
            },

            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(slf)
    } else {
        Err(Error { errors })
    }
}

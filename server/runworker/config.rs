use std::env;
use std::error;
use std::net;

pub struct Config {
    pub output_addr: net::SocketAddr,
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

impl Default for Config {
    fn default() -> Self {
        let bind = net::IpAddr::V4(net::Ipv4Addr::new(0, 0, 0, 0));
        Self {
            output_addr: net::SocketAddr::new(bind, 9001),
        }
    }
}

pub fn from_env() -> Result<Config, Error> {
    let mut slf = Config::default();
    let mut errors = vec![];
    let mut cfg = Config::default();

    for (key, val) in env::vars() {
        match key.as_ref() {
            "MYSTIC_OUTPUT_ADDR" => match val.parse() {
                Err(err) => errors.push(EnvError {
                    env_var: key.to_owned(),
                    env_val: val.to_owned(),
                    error: Box::new(err),
                }),
                Ok(output_addr) => {
                    cfg.output_addr = output_addr;
                }
            },
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(cfg)
    } else {
        Err(Error { errors })
    }
}

use std::io;
use std::result;

use super::config;

#[derive(Debug)]
pub struct IoError {
    pub action: &'static str,
    pub error: io::Error,
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Config(config::Error),
    InvalidArgs,
}

pub type Result<T> = result::Result<T, Error>;

impl From<config::Error> for Error {
    fn from(cfg_err: config::Error) -> Self {
        Error::Config(cfg_err)
    }
}

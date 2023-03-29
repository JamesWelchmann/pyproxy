use std::io;
use std::result;

use super::config;

#[derive(Debug)]
pub struct IoError {
    pub action: &'static str,
    pub err: io::Error,
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Config(config::Error),
    Protocol(protocol::Error),
}

pub type Result<T> = result::Result<T, Error>;

pub fn fatal_io_err<T>(action: &'static str, io_res: io::Result<T>) -> Result<T> {
    io_res.map_err(|err| Error::Io(IoError { action, err }))
}

impl From<config::Error> for Error {
    fn from(cfg_err: config::Error) -> Self {
        Error::Config(cfg_err)
    }
}

impl From<protocol::Error> for Error {
    fn from(err: protocol::Error) -> Self {
        Error::Protocol(err)
    }
}

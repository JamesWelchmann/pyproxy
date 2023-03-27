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
    StreamClosed,
    Protocol(protocol::Error),
}

pub type Result<T> = result::Result<T, Error>;

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

pub fn io_error(action: &'static str, error: io::Error) -> Error {
    Error::Io(IoError { action, error })
}

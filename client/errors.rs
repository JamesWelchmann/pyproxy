use std::io;
use std::result;
use std::any::Any;

#[derive(Debug)]
pub struct IoError {
    pub action: &'static str,
    pub error: io::Error,
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    ServerDidntSendHello,
    MissingMainStream,
    Protocol(protocol::Error),
    ClientThreadDoesNotExist,
    ThreadClosed(Box<dyn Any + Send + 'static>),
    OutputStreamClosed,
}

pub type Result<T> = result::Result<T, Error>;

pub fn io_error(action: &'static str, error: io::Error) -> Error {
    Error::Io(IoError { action, error })
}

pub fn fatal_io_error<T>(action: &'static str, io_res: io::Result<T>) -> Result<T> {
    io_res.map_err(|e| io_error(action, e))
}

impl From<protocol::Error> for Error {
    fn from(err: protocol::Error) -> Self {
        Error::Protocol(err)
    }
}

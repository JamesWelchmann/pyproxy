use std::io;
use std::result;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    ServerDidntSendHello,
    MissingMainStream,
    Protocol(protocol::Error),
}

pub type Result<T> = result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<protocol::Error> for Error {
    fn from(err: protocol::Error) -> Self {
        Error::Protocol(err)
    }
}

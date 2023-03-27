use std::result;

use super::MessageType;

#[derive(Debug)]
pub enum Error {
    WrongVersion(u8),
    UnrecognisedMessageType(u8),
    FailedDeserialze(bincode::Error),
    UnexpectedMessageType(MessageType),
    Deserialize(bincode::Error),
}

pub type Result<T> = result::Result<T, Error>;

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Deserialize(err)
    }
}

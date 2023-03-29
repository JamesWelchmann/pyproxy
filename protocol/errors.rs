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

impl Error {
  pub fn reason(&self) -> String {
    match self {
      Error::WrongVersion(version) => format!("unsupported protocol version {}", version),
      Error::UnrecognisedMessageType(msg_type) => format!("unrecognised message type {}", msg_type),
      Error::FailedDeserialze(err) => format!("failed to deserialize message {:?}", err),
      Error::UnexpectedMessageType(msg_type) => format!("message type invalid here {:?}", msg_type),
      Error::Deserialize(err) => format!("failed to deserialize message {:?}", err),
    }
  }
}

pub type Result<T> = result::Result<T, Error>;

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Deserialize(err)
    }
}

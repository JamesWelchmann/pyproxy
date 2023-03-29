use super::{Error, Result};

pub const HEADER_SIZE: usize = 5;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClientHello {
    pub stream_token: String,
}

pub fn read_hello(body: &[u8]) -> Result<ClientHello> {
    let client_hello = bincode::deserialize(body)?;
    Ok(client_hello)
}

#[derive(Copy, Clone, Debug)]
pub enum MessageType {
    Stdout,
    Stderr,
}

impl MessageType {
    fn as_u8(self) -> u8 {
        match self {
            MessageType::Stdout => 1,
            MessageType::Stderr => 2,
        }
    }

    fn from_u8(b: u8) -> Result<Self> {
        match b {
            1 => Ok(MessageType::Stdout),
            2 => Ok(MessageType::Stderr),
            _ => Err(Error::UnrecognisedMessageType(b)),
        }
    }
}

pub struct MessageHeader {
    pub msg_type: MessageType,
    pub msg_len: usize,
}

impl MessageHeader {
    pub fn new(msg_type: MessageType, msg_len: usize) -> Self {
        Self { msg_type, msg_len }
    }

    fn into_buf(self) -> [u8; HEADER_SIZE] {
        let len = (self.msg_len as u32).to_be_bytes();
        [self.msg_type.as_u8(), len[0], len[1], len[2], len[3]]
    }

    pub fn from_raw(raw: [u8; HEADER_SIZE]) -> Result<Self> {
        let msg_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;

        Ok(Self {
            msg_type: MessageType::from_u8(raw[0])?,
            msg_len,
        })
    }
}

pub fn new_msg(header: MessageHeader, data: &[u8]) -> Vec<u8> {
    let mut ans = Vec::with_capacity(data.len() + HEADER_SIZE);
    ans.extend(&header.into_buf());
    ans.extend(data);
    ans
}

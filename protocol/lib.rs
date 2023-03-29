mod errors;
pub use errors::{Error, Result};
pub mod mainstream;
pub use mainstream::{CodePickle, CodeString, ResponseCodePickle, ResponseCodeString};
pub mod outputstream;

pub const VERSION: u8 = 0;
pub const REQUEST_HEADER_SIZE: usize = 8;
pub const RESPONSE_HEADER_SIZE: usize = 12;
pub const SESSION_ID_LENGTH: usize = 16;

// 8 byte message header
pub struct RequestMessageHeader {
    pub zero: u8,
    pub version: u8,
    pub msg_type: MessageType,
    pub msg_sub_type: u8,

    // BE encoded
    pub msg_len: [u8; 4],
}

#[derive(Copy, Clone, Debug)]
pub enum MessageType {
    Hello,
    CodeString,
    CodePickle,
}

#[derive(Debug)]
pub enum RequestMessage {
    Hello(RequestClientHello),
    CodeString(CodeString),
    CodePickle(CodePickle),
}

#[derive(Debug)]
pub enum ResponseMessage {
    Hello(ResponseClientHello),
    CodeString(ResponseCodePickle),
    CodePickle(ResponseCodeString),
}

impl ResponseMessage {
    pub fn is_hello(&self) -> bool {
        match self {
            ResponseMessage::Hello(_) => true,
            _ => false,
        }
    }

    pub fn future_id<'s>(&'s self) -> &'s str {
        match self {
            ResponseMessage::Hello(_) => "000000",
            ResponseMessage::CodeString(s) => &s.future_id,
            ResponseMessage::CodePickle(s) => &s.future_id,
        }
    }
}

impl RequestMessage {
    pub fn future_id(&self) -> Option<&str> {
        match self {
            RequestMessage::Hello(_) => None,
            RequestMessage::CodeString(s) => Some(&s.future_id),
            RequestMessage::CodePickle(s) => Some(&s.future_id),
        }
    }
}

pub fn new_req<T: serde::Serialize>(msg_type: MessageType, msg_sub_type: u8, msg: T) -> Vec<u8> {
    let payload = bincode::serialize(&msg).expect("couldn't serialize request message");
    let header = RequestMessageHeader::new(msg_type, msg_sub_type, payload.len());
    let mut msg = Vec::with_capacity(payload.len() + REQUEST_HEADER_SIZE);
    msg.extend(&header.into_buf());
    msg.extend(&payload);
    msg
}

pub fn read_req(header: RequestMessageHeader, body: &[u8]) -> Result<RequestMessage> {
    match header.msg_type {
        MessageType::Hello => Ok(RequestMessage::Hello(RequestClientHello {})),
        MessageType::CodeString => Ok(RequestMessage::CodeString(bincode::deserialize(body)?)),
        MessageType::CodePickle => Ok(RequestMessage::CodePickle(bincode::deserialize(body)?)),
    }
}

impl MessageType {
    fn as_u8(self) -> u8 {
        match self {
            MessageType::Hello => 1,
            MessageType::CodeString => 2,
            MessageType::CodePickle => 3,
        }
    }

    fn from_u8(b: u8) -> Result<Self> {
        match b {
            1 => Ok(MessageType::Hello),
            2 => Ok(MessageType::CodeString),
            3 => Ok(MessageType::CodePickle),
            _ => Err(Error::UnrecognisedMessageType(b)),
        }
    }
}

impl RequestMessageHeader {
    pub fn new(msg_type: MessageType, msg_sub_type: u8, msg_len: usize) -> Self {
        Self {
            zero: 0,
            version: VERSION,
            msg_type: msg_type,
            msg_sub_type,
            msg_len: (msg_len as u32).to_be_bytes(),
        }
    }

    pub fn into_buf(self) -> [u8; REQUEST_HEADER_SIZE] {
        [
            self.zero,
            self.version,
            self.msg_type.as_u8(),
            self.msg_sub_type,
            self.msg_len[0],
            self.msg_len[1],
            self.msg_len[2],
            self.msg_len[3],
        ]
    }

    pub fn msg_len(&self) -> usize {
        (u32::from_be_bytes(self.msg_len)) as usize
    }

    pub fn from_buf(buf: [u8; REQUEST_HEADER_SIZE]) -> Result<Self> {
        if buf[1] != VERSION {
            return Err(Error::WrongVersion(buf[1]));
        }

        Ok(Self {
            zero: 0,
            version: buf[1],
            msg_type: MessageType::from_u8(buf[2])?,
            msg_sub_type: buf[3],
            msg_len: [buf[4], buf[5], buf[6], buf[7]],
        })
    }
}

pub fn read_response(header: ResponseMessageHeader, body: &[u8]) -> Result<ResponseMessage> {
    match header.msg_type {
        MessageType::Hello => Ok(ResponseMessage::Hello(read_msg(body)?)),
        MessageType::CodeString => Ok(ResponseMessage::CodeString(read_msg(body)?)),
        MessageType::CodePickle => Ok(ResponseMessage::CodePickle(read_msg(body)?)),
    }
}

pub struct ResponseMessageHeader {
    pub zero: u8,
    pub version: u8,
    pub msg_type: MessageType,
    pub msg_sub_type: u8,

    // BE encoded
    pub msg_len: [u8; 4],

    // BE encoded sequence number
    pub seq_num: [u8; 4],
}

impl ResponseMessageHeader {
    pub fn new(msg_type: MessageType, msg_sub_type: u8, msg_len: usize, seq_num: u32) -> Self {
        Self {
            zero: 0,
            version: VERSION,
            msg_type,
            msg_sub_type,
            msg_len: (msg_len as u32).to_be_bytes(),
            seq_num: seq_num.to_be_bytes(),
        }
    }

    pub fn into_buf(self) -> [u8; RESPONSE_HEADER_SIZE] {
        [
            self.zero,
            self.version,
            self.msg_type.as_u8(),
            self.msg_sub_type,
            self.msg_len[0],
            self.msg_len[1],
            self.msg_len[2],
            self.msg_len[3],
            self.seq_num[0],
            self.seq_num[1],
            self.seq_num[2],
            self.seq_num[3],
        ]
    }

    pub fn from_buf(buf: [u8; RESPONSE_HEADER_SIZE]) -> Result<Self> {
        let header = Self {
            zero: 0,
            version: buf[1],
            msg_type: MessageType::from_u8(buf[2])?,
            msg_sub_type: buf[3],
            msg_len: [buf[4], buf[5], buf[6], buf[7]],
            seq_num: [buf[8], buf[9], buf[10], buf[11]],
        };

        if header.version != VERSION {
            return Err(Error::WrongVersion(header.version));
        }

        Ok(header)
    }

    pub fn msg_len(&self) -> usize {
        (u32::from_be_bytes(self.msg_len)) as usize
    }
}

pub fn read_msg<'a, T: serde::Deserialize<'a>>(buf: &'a [u8]) -> Result<T> {
    bincode::deserialize(buf).map_err(Error::FailedDeserialze)
}

pub struct ServerPushHeader {
    pub msg_type: u8,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RequestClientHello {}

impl RequestClientHello {
    pub fn new() -> Self {
        Self {}
    }

    pub fn into_buf(self) -> Vec<u8> {
        vec![]
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ResponseClientHello {
    // session_id generated by server
    pub session_id: String,

    // token used to connect to output stream
    pub stream_token: String,

    // token DNS + port
    pub output_addr: String,
}

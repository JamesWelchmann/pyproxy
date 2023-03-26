// Messages from worker to master
use std::collections::HashMap;
use std::time;

pub const LOG_MESSAGE: u8 = 1;

// Header is always five bytes, message type follow by 4 byte msg len

#[derive(serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LogMessage {
    pub level: LogLevel,
    pub msg: String,
    pub ts: String,
    pub tags: HashMap<String, serde_json::Value>,
}

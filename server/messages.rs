// Messages from worker to master
use std::collections::HashMap;

pub const LOG_MESSAGE: u8 = 1;
pub const PRINT_MESSAGE: u8 = 2;

// Header is always five bytes, message type follow by 4 byte msg len

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum LogValue {
    String(String),
    Uint(u64),
    Int(i64),
    Bool(bool),
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct LogMessage {
    pub level: LogLevel,
    pub msg: String,
    pub ts: String,
    pub tags: Vec<(String, LogValue)>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PrintMessage {
    pub message: String,
}

pub const NEW_REQUEST_START: &'static str =
    "8b588b6fbb7eaa6a66da438c0dc1cced45c9c55cdf1eb137ba133ba1d7d95b5b";
pub const NEW_REQUEST_END: &'static str =
    "962375a5e9ffb94b822a69902f462e3394b33a51fbd17d9639cd0f6a9640268d";

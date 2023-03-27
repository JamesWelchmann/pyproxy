#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CodeString {
    pub future_id: String,
    pub code: String,
    pub locals: Vec<u8>,
    pub globals: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CodePickle {
    pub future_id: String,
    pub pickle: Vec<u8>,
    pub locals: Vec<u8>,
    pub globals: Vec<u8>,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WsMessage {
    Passing(Passing),
    Status { event: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Passing {
    pub passing_number: u32,
    pub transponder: String,
    pub rtc_time: String,
    pub strength: u32,
    pub tran_code: String,
    pub noise: u32,
    pub hits: u32,
    pub date: String,
    pub time: String,
}

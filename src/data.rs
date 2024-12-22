use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub enum SerialDirection {
    Send,
    Receive,
}

impl fmt::Display for SerialDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SerialDirection::Send => write!(f, "SEND"),
            SerialDirection::Receive => write!(f, "RECV"),
        }
    }
}

pub fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub relative_time: f64,
    pub absolute_time: f64,
    pub direction: SerialDirection,
    pub payload: String,
}

impl Default for Packet {
    fn default() -> Packet {
        Packet {
            relative_time: 0.0,
            absolute_time: get_epoch_ms() as f64,
            direction: SerialDirection::Send,
            payload: "".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataContainer {
    pub time: Vec<f64>,
    pub absolute_time: Vec<f64>,
    pub dataset: Vec<Vec<f32>>,
    pub raw_traffic: Vec<Packet>,
    pub loaded_from_file: bool,
}

impl Default for DataContainer {
    fn default() -> DataContainer {
        DataContainer {
            time: vec![],
            absolute_time: vec![],
            dataset: vec![vec![]],
            raw_traffic: vec![],
            loaded_from_file: false,
        }
    }
}

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum SerialDirection {
    SEND,
    RECEIVE,
}

impl fmt::Display for SerialDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SerialDirection::SEND => write!(f, "SEND"),
            SerialDirection::RECEIVE => write!(f, "RECV"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub time: u128,
    pub direction: SerialDirection,
    pub payload: String,
}

impl Default for Packet {
    fn default() -> Packet {
        Packet {
            time: 0,
            direction: SerialDirection::SEND,
            payload: "".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataContainer {
    pub time: Vec<u128>,
    pub dataset: Vec<Vec<f32>>,
    pub raw_traffic: Vec<Packet>,
}

impl Default for DataContainer {
    fn default() -> DataContainer {
        DataContainer {
            time: vec![],
            dataset: vec![vec![]],
            raw_traffic: vec![],
        }
    }
}
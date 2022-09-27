use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use itertools_num::linspace;
use serde::{Serialize, Deserialize};
use crate::gui::{print_to_console, Print, update_in_console};

const BUF_LEN: usize = 1024;
const READ_HEADER_LEN: usize = 19;

#[derive(Clone)]
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

#[derive(Clone)]
pub struct Packet {
    pub time: Instant,
    pub direction: SerialDirection,
    pub payload: String,
}

impl Default for Packet {
    fn default() -> Packet {
        return Packet {
            time: Instant::now(),
            direction: SerialDirection::SEND,
            payload: "".to_string()
        }
    }
}

#[derive(Clone)]
pub struct DataContainer {
    pub time: Vec<f32>,
    pub dataset: Vec<Vec<f32>>,
    pub raw_traffic: Vec<Packet>,
}

impl Default for DataContainer {
    fn default() -> DataContainer {
        return DataContainer {
            time: vec![],
            dataset: vec![vec![]],
            raw_traffic: vec![],
        };
    }
}
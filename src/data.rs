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

#[derive(Clone, Serialize, Deserialize)]
pub struct DataContainer {
    pub time: Vec<f32>,
    pub dataset: Vec<Vec<f32>>,
}

impl Default for DataContainer {
    fn default() -> DataContainer {
        return DataContainer {
            time: vec![],
            dataset: vec![vec![]]
        };
    }
}
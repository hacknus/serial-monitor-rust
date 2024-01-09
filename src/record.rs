use csv::Writer;
use serde::{Deserialize, Serialize};

use crate::data::DataContainer;
use crate::gui::{print_to_console, Print};
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordOptions {
    pub enable: bool,
    pub record_path: PathBuf,
    pub windows_style_line_endings: bool,
    pub write_header_line: bool,
    pub insert_timestamp: bool,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            enable: false,
            record_path: PathBuf::new(),
            windows_style_line_endings: false,
            write_header_line: true,
            insert_timestamp: true,
        }
    }
}

pub struct RecordData {
    pub time: u128,
    pub datas: Vec<f64>,
}

fn get_headers(
    data_lock: &Arc<RwLock<DataContainer>>,
    record_options: &RecordOptions,
) -> Vec<String> {
    let mut headers = vec![];
    if record_options.insert_timestamp {
        headers.push("Timestamp".to_owned());
    }
    if let Ok(read_guard) = data_lock.read() {
        for name in &read_guard.names {
            headers.push(name.clone());
        }
    }
    headers
}

pub fn record_thread(
    data_lock: Arc<RwLock<DataContainer>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    record_options_rx: Receiver<RecordOptions>,
    record_data_rx: Receiver<RecordData>,
) {
    let mut record_options = RecordOptions::default();
    let mut wtr: Option<Writer<File>> = None;
    'record_loop: loop {
        if let Ok(opt) = record_options_rx.try_recv() {
            record_options = opt;
        }

        if record_options.enable {
            if wtr.is_none() {
                fs::remove_file(&record_options.record_path).unwrap_or_default();
                wtr = match Writer::from_path(&record_options.record_path) {
                    Ok(w) => Some(w),
                    Err(e) => {
                        print_to_console(
                            &print_lock,
                            Print::Error(format!("Error while create recorder: {:?}", e)),
                        );
                        record_options.enable = false;
                        continue;
                    }
                };
                if let Some(w) = &mut wtr {
                    let headers = get_headers(&data_lock, &record_options);
                    if let Err(e) = w.write_record(&headers) {
                        print_to_console(
                            &print_lock,
                            Print::Error(format!("Error while create headers: {:?}", e)),
                        );
                    };
                }
            }
            let datas_vec = match record_data_rx.try_recv() {
                Ok(datas) => {
                    let mut dv = vec![];
                    if record_options.insert_timestamp {
                        dv.push(datas.time.to_string())
                    }
                    for data in datas.datas {
                        dv.push(data.to_string())
                    }
                    dv
                }
                Err(_) => continue 'record_loop,
            };
            if let Some(w) = &mut wtr {
                if let Err(e) = w.write_record(&datas_vec) {
                    print_to_console(
                        &print_lock,
                        Print::Error(format!("Error while record data: {:?}", e)),
                    );
                }
                w.flush().unwrap_or_default();
            } else {
                record_options.enable = false;
                continue;
            }
        } else {
            wtr = None;
            let _recv = record_data_rx.try_recv();
        }
    }
}

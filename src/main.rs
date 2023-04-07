#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use std::cmp::max;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, RwLock};
use std::thread;
use std::time::Duration;

use eframe::egui::{vec2, Visuals};
use preferences::AppInfo;

use crate::data::{DataContainer, Packet};
use crate::gui::{load_gui_settings, print_to_console, MyApp, Print};
use crate::io::save_to_csv;
use crate::serial::serial_thread;

mod data;
mod gui;
mod io;
mod serial;
mod toggle;

const APP_INFO: AppInfo = AppInfo {
    name: "Serial Monitor",
    author: "Linus Leo Stöckli",
};
const PREFS_KEY: &str = "config/gui";

#[derive(Default, Debug)]
pub struct Device {
    pub name: String,
    pub baud_rate: u32,
}

/// A set of options for saving data to a CSV file.
#[derive(Debug)]
pub struct CsvOptions {
    file_path: PathBuf,
    save_absolute_time: bool,
}

fn split(payload: &str) -> Vec<f32> {
    let mut split_data: Vec<&str> = vec![];
    for s in payload.split(':') {
        split_data.extend(s.split(','));
    }
    split_data
        .iter()
        .map(|x| x.trim())
        .flat_map(|x| x.parse::<f32>())
        .collect()
}

fn main_thread(
    data_lock: Arc<RwLock<DataContainer>>,
    raw_data_lock: Arc<RwLock<Vec<Packet>>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    names_rx: Receiver<Vec<String>>,
    save_rx: Receiver<CsvOptions>,
    clear_rx: Receiver<bool>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataContainer::default();
    let mut failed_format_counter = 0;
    loop {
        if let Ok(cl) = clear_rx.recv_timeout(Duration::from_millis(10)) {
            if cl {
                data = DataContainer::default();
                failed_format_counter = 0;
            }
        }

        if let Ok(names) = names_rx.recv_timeout(Duration::from_millis(10)) {
            if data.names.len() == names.len() {
                data.names = names;
            }
        }

        if let Ok(read_guard) = raw_data_lock.read() {
            for packet in read_guard.iter() {
                if !packet.payload.is_empty() {
                    data.raw_traffic.push(packet.clone());
                    let split_data = split(&packet.payload);
                    if data.dataset.is_empty() || failed_format_counter > 10 {
                        // resetting dataset
                        data.dataset = vec![vec![]; max(split_data.len(), 1)];
                        data.names = (0..max(split_data.len(), 1))
                            .map(|i| format!("Column {i}"))
                            .collect();
                        failed_format_counter = 0;
                        // println!("resetting dataset. split length = {}, length data.dataset = {}", split_data.len(), data.dataset.len());
                    } else if split_data.len() == data.dataset.len() {
                        // appending data
                        for (i, set) in data.dataset.iter_mut().enumerate() {
                            set.push(split_data[i]);
                            failed_format_counter = 0;
                        }
                        data.time.push(packet.relative_time);
                        data.absolute_time.push(packet.absolute_time);
                        if data.time.len() != data.dataset[0].len() {
                            // resetting dataset
                            data.time = vec![];
                            data.dataset = vec![vec![]; max(split_data.len(), 1)];
                            data.names = (0..max(split_data.len(), 1))
                                .map(|i| format!("Column {i}"))
                                .collect();
                        }
                    } else {
                        // not same length
                        failed_format_counter += 1;
                        // println!("not same length in main! length split_data = {}, length data.dataset = {}", split_data.len(), data.dataset.len())
                    }
                }
            }
        }
        if let Ok(mut write_guard) = raw_data_lock.write() {
            *write_guard = vec![Packet::default()];
        }

        if let Ok(csv_options) = save_rx.recv_timeout(Duration::from_millis(10)) {
            match save_to_csv(&data, &csv_options) {
                Ok(_) => {
                    print_to_console(
                        &print_lock,
                        Print::Ok(format!("saved data file to {:?} ", csv_options.file_path)),
                    );
                }
                Err(e) => {
                    print_to_console(
                        &print_lock,
                        Print::Error(format!(
                            "failed to save file to {:?}: {:?}",
                            csv_options.file_path, e
                        )),
                    );
                }
            }
        }

        if let Ok(mut write_guard) = data_lock.write() {
            *write_guard = data.clone();
        }
        // std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    let gui_settings = load_gui_settings();

    let device_lock = Arc::new(RwLock::new(Device::default()));
    let devices_lock = Arc::new(RwLock::new(vec![gui_settings.device.clone()]));
    let raw_data_lock = Arc::new(RwLock::new(vec![Packet::default()]));
    let data_lock = Arc::new(RwLock::new(DataContainer::default()));
    let print_lock = Arc::new(RwLock::new(vec![Print::Empty]));
    let connected_lock = Arc::new(RwLock::new(false));

    let (save_tx, save_rx): (Sender<CsvOptions>, Receiver<CsvOptions>) = mpsc::channel();
    let (send_tx, send_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (clear_tx, clear_rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let (names_tx, names_rx): (Sender<Vec<String>>, Receiver<Vec<String>>) = mpsc::channel();

    let serial_device_lock = device_lock.clone();
    let serial_devices_lock = devices_lock.clone();
    let serial_raw_data_lock = raw_data_lock.clone();
    let serial_print_lock = print_lock.clone();
    let serial_connected_lock = connected_lock.clone();

    println!("starting connection thread..");
    let _serial_thread_handler = thread::spawn(|| {
        serial_thread(
            send_rx,
            serial_device_lock,
            serial_devices_lock,
            serial_raw_data_lock,
            serial_print_lock,
            serial_connected_lock,
        );
    });

    let main_data_lock = data_lock.clone();
    let main_raw_data_lock = raw_data_lock;
    let main_print_lock = print_lock.clone();

    println!("starting main thread..");
    let _main_thread_handler = thread::spawn(|| {
        main_thread(
            main_data_lock,
            main_raw_data_lock,
            main_print_lock,
            names_rx,
            save_rx,
            clear_rx,
        );
    });

    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Option::from(vec2(gui_settings.x, gui_settings.y)),
        ..Default::default()
    };

    let gui_data_lock = data_lock;
    let gui_device_lock = device_lock;
    let gui_devices_lock = devices_lock;
    let gui_connected_lock = connected_lock;
    let gui_print_lock = print_lock;

    if let Err(e) = eframe::run_native(
        "Serial Monitor",
        options,
        Box::new(|_cc| {
            _cc.egui_ctx.set_visuals(Visuals::dark());
            Box::new(MyApp::new(
                gui_print_lock,
                gui_data_lock,
                gui_device_lock,
                gui_devices_lock,
                gui_connected_lock,
                gui_settings,
                names_tx,
                save_tx,
                send_tx,
                clear_tx,
            ))
        }),
    ) {
        println!("error: {e:?}");
    }
}

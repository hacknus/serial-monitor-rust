#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

mod data;
mod gui;
mod io;
mod serial;
mod toggle;

use crate::data::{DataContainer, Packet};
use eframe::egui::{vec2, Visuals};
use preferences::{AppInfo, Preferences};
use std::cmp::max;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, RwLock};
use std::thread;
use std::time::Duration;

use crate::gui::{print_to_console, update_in_console, GuiSettingsContainer, MyApp, Print};
use crate::io::save_to_csv;
use crate::serial::serial_thread;

const APP_INFO: AppInfo = AppInfo {
    name: "Serial Monitor",
    author: "Linus Leo Stöckli",
};

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
    save_rx: Receiver<PathBuf>,
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
        if let Ok(read_guard) = raw_data_lock.read() {
            let packets = read_guard.clone();
            for packet in packets.iter() {
                if !packet.payload.is_empty() {
                    data.raw_traffic.push(packet.clone());
                    let split_data = split(&packet.payload);
                    if data.dataset.is_empty() || failed_format_counter > 10 {
                        data.dataset = vec![vec![]; max(split_data.len(), 1)];
                        failed_format_counter = 0;
                        // println!("resetting dataset. split length = {}, length data.dataset = {}", split_data.len(), data.dataset.len());
                    } else if split_data.len() == data.dataset.len() {
                        for (i, set) in data.dataset.iter_mut().enumerate() {
                            set.push(split_data[i]);
                            failed_format_counter = 0;
                        }
                        data.time.push(packet.time);
                        if data.time.len() != data.dataset[0].len() {
                            data.time = vec![];
                            data.dataset = vec![vec![]; max(split_data.len(), 1)];
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

        if let Ok(file_path) = save_rx.recv_timeout(Duration::from_millis(10)) {
            let print_index = print_to_console(
                &print_lock,
                Print::Task(format!("saving data file to {:?} ...", file_path)),
            );
            match save_to_csv(&data, &file_path) {
                Ok(_) => {
                    update_in_console(
                        &print_lock,
                        Print::OK(format!("saved data file to {:?} ", file_path)),
                        print_index,
                    );
                }
                Err(e) => {
                    print_to_console(
                        &print_lock,
                        Print::Error(format!("failed to save file: {e:?}")),
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
    let mut gui_settings = GuiSettingsContainer::default();
    let prefs_key = "config/gui";
    if let Ok(load_result) = GuiSettingsContainer::load(&APP_INFO, prefs_key) {
        gui_settings = load_result;
    } else {
        // save default settings
        if gui_settings.save(&APP_INFO, prefs_key).is_err() {
            println!("failed to save gui_settings");
        }
    }

    let device_lock = Arc::new(RwLock::new(gui_settings.device.clone()));
    let devices_lock = Arc::new(RwLock::new(vec![gui_settings.device.clone()]));
    let baud_lock = Arc::new(RwLock::new(gui_settings.baud));
    let raw_data_lock = Arc::new(RwLock::new(vec![Packet::default()]));
    let data_lock = Arc::new(RwLock::new(DataContainer::default()));
    let print_lock = Arc::new(RwLock::new(vec![Print::Empty]));
    let connected_lock = Arc::new(RwLock::new(false));

    let (save_tx, save_rx): (Sender<PathBuf>, Receiver<PathBuf>) = mpsc::channel();
    let (send_tx, send_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (clear_tx, clear_rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

    let serial_device_lock = device_lock.clone();
    let serial_devices_lock = devices_lock.clone();
    let serial_baud_lock = baud_lock.clone();
    let serial_raw_data_lock = raw_data_lock.clone();
    let serial_print_lock = print_lock.clone();
    let serial_connected_lock = connected_lock.clone();
    let serial_gui_settings = gui_settings.clone();

    println!("starting connection thread..");
    let serial_thread = thread::spawn(|| {
        serial_thread(
            serial_gui_settings,
            send_rx,
            serial_device_lock,
            serial_devices_lock,
            serial_baud_lock,
            serial_raw_data_lock,
            serial_print_lock,
            serial_connected_lock,
        );
    });

    let main_data_lock = data_lock.clone();
    let main_raw_data_lock = raw_data_lock.clone();
    let main_print_lock = print_lock.clone();

    println!("starting main thread..");
    let main_thread_handler = thread::spawn(|| {
        main_thread(
            main_data_lock,
            main_raw_data_lock,
            main_print_lock,
            save_rx,
            clear_rx,
        );
    });

    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Option::from(vec2(gui_settings.x, gui_settings.y)),
        ..Default::default()
    };

    let gui_data_lock = data_lock.clone();
    let gui_device_lock = device_lock.clone();
    let gui_devices_lock = devices_lock.clone();
    let gui_baud_lock = baud_lock.clone();
    let gui_connected_lock = connected_lock.clone();
    let gui_print_lock = print_lock.clone();

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
                gui_baud_lock,
                gui_connected_lock,
                gui_settings,
                save_tx,
                send_tx,
                clear_tx,
            ))
        }),
    ) {
        println!("error: {e:?}");
    }
}

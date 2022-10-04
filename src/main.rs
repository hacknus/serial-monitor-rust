#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate serde;
extern crate preferences;
extern crate core;
extern crate csv;

mod gui;
mod toggle;
mod io;
mod serial;
mod data;

use std::thread;
use eframe::egui::{vec2, Visuals};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, mpsc, RwLock};
use std::time::{Duration};
use preferences::{AppInfo, Preferences};
use crate::data::{DataContainer, Packet};

use crate::gui::{GuiSettingsContainer, MyApp, Print, print_to_console, update_in_console};
use crate::io::save_to_csv;
use crate::serial::serial_thread;

const APP_INFO: AppInfo = AppInfo { name: "Serial Monitor", author: "Linus Leo Stöckli" };

fn split(payload: &str) -> Vec<&str> {
    let delimiter_1;
    if payload.contains(": "){
        delimiter_1 = ": ";
    } else {
        delimiter_1 = ":";
    }
    let delimiter_2;
    if payload.contains(", "){
        delimiter_2 = ", ";
    } else {
        delimiter_2 = ",";
    }
    let mut split_data: Vec<&str> = vec![];
    let first_split = payload.split(delimiter_1).collect::<Vec<&str>>();
    for s in first_split.iter() {
        let s_split = s.split(delimiter_2).collect::<Vec<&str>>();
        for si in s_split.iter() {
            let mut contains_value = false;
            for char in si.bytes(){
                if b"-0.123456789".contains(&char) {
                    contains_value = true;
                    break;
                }
            }
            if contains_value {
                split_data.push(si);
            }

        }
    }
    split_data
}

fn main_thread(data_lock: Arc<RwLock<DataContainer>>,
               raw_data_lock: Arc<RwLock<Vec<Packet>>>,
               print_lock: Arc<RwLock<Vec<Print>>>,
               save_rx: Receiver<String>,
               clear_rx: Receiver<bool>) {
    // reads data from mutex, samples and saves if needed
    let mut acquire = false;
    let mut file_path = "serial_monitor_test.csv".to_string();
    let mut data = DataContainer::default();
    let mut failed_format_counter = 0;
    loop {
        match clear_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(cl) => {
                if cl {
                    data = DataContainer::default();
                    failed_format_counter = 0;
                }
            }
            Err(..) => ()
        }
        if let Ok(read_guard) = raw_data_lock.read() {
            let packets = read_guard.clone();
            for packet in packets.iter() {
                if packet.payload == "".to_string() {
                    // empty dataset
                } else {
                    data.raw_traffic.push(packet.clone());
                    let split_data = split(packet.payload);
                    if data.dataset.len() == 0 || failed_format_counter > 10 {
                        data.dataset = vec![vec![]; split_data.len()];
                        failed_format_counter = 0;
                        // println!("resetting dataset. split length = {}, length data.dataset = {}", split_data.len(), data.dataset.len());
                    } else {
                        if split_data.len() == data.dataset.len() {
                            let mut parse_state = false;
                            for (i, set) in data.dataset.iter_mut().enumerate() {
                                match split_data[i].parse::<f32>() {
                                    Ok(r) => {
                                        // println!("success parsing i={i}");
                                        set.push(r);
                                        parse_state = true;
                                        failed_format_counter = 0;
                                    }
                                    Err(_) => {
                                        // println!("failed to parse i={i}");
                                        set.push(f32::NAN);
                                        failed_format_counter += 1;
                                    }
                                }
                            }
                            if parse_state {
                                data.time.push(packet.time);
                            }
                        } else {
                            // not same length
                            failed_format_counter += 1;
                            // println!("not same length in main! length split_data = {}, length data.dataset = {}", split_data.len(), data.dataset.len())
                        }
                    }
                }
            }
        }
        if let Ok(mut write_guard) = raw_data_lock.write() {
            *write_guard = vec![Packet::default()];
        }

        match save_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(fp) => {
                file_path = fp;
                acquire = true
            }
            Err(..) => ()
        }

        if acquire == true {
            // save file
            let print_index = print_to_console(&print_lock, Print::TASK(format!("saving data file to {:?} ...", file_path).to_string()));
            let save_result = save_to_csv(&data, &file_path);
            match save_result {
                Ok(_) => {
                    update_in_console(&print_lock, Print::OK(format!("saved data file to {:?} ", file_path).to_string()), print_index);
                }
                Err(e) => {
                    print_to_console(&print_lock, Print::ERROR(format!("failed to save file: {e:?}").to_string()));
                }
            }
            acquire = false;
        }

        if let Ok(mut write_guard) = data_lock.write() {
            *write_guard = data.clone();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    let mut gui_settings = GuiSettingsContainer::default();
    let prefs_key = "config/gui";
    let load_result = GuiSettingsContainer::load(&APP_INFO, prefs_key);
    if load_result.is_ok() {
        gui_settings = load_result.unwrap();
    } else {
        // save default settings
        match gui_settings.save(&APP_INFO, prefs_key) {
            Ok(_) => {}
            Err(_) => {
                println!("failed to save gui_settings");
            }
        }
    }

    let device_lock = Arc::new(RwLock::new(gui_settings.device.clone()));
    let devices_lock = Arc::new(RwLock::new(vec![gui_settings.device.clone()]));
    let baud_lock = Arc::new(RwLock::new(gui_settings.baud.clone()));
    let raw_data_lock = Arc::new(RwLock::new(vec![Packet::default()]));
    let data_lock = Arc::new(RwLock::new(DataContainer::default()));
    let print_lock = Arc::new(RwLock::new(vec![Print::EMPTY]));
    let connected_lock = Arc::new(RwLock::new(false));

    let (save_tx, save_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
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
        serial_thread(serial_gui_settings,
                      send_rx,
                      serial_device_lock,
                      serial_devices_lock,
                      serial_baud_lock,
                      serial_raw_data_lock,
                      serial_print_lock,
                      serial_connected_lock);
    });

    let main_data_lock = data_lock.clone();
    let main_raw_data_lock = raw_data_lock.clone();
    let main_print_lock = print_lock.clone();

    println!("starting main thread..");
    let main_thread_handler = thread::spawn(|| {
        main_thread(main_data_lock,
                    main_raw_data_lock,
                    main_print_lock,
                    save_rx,
                    clear_rx);
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

    eframe::run_native(
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
    );


    main_thread_handler.join().unwrap();
    serial_thread.join().unwrap();
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use crate::data::{DataContainer, Packet, SerialDirection};
use crate::gui::{load_gui_settings, MyApp, RIGHT_PANEL_WIDTH};
use crate::io::{open_from_csv, save_to_csv, FileOptions};
use crate::serial::{load_serial_settings, serial_thread, Device};
use eframe::egui::{vec2, ViewportBuilder, Visuals};
use eframe::{egui, icon_data};
use preferences::AppInfo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, RwLock};
use std::{env, thread};

mod color_picker;
mod custom_highlighter;
mod data;
mod gui;
mod io;
mod serial;
mod settings_window;
mod toggle;
mod update;

const APP_INFO: AppInfo = AppInfo {
    name: "Serial Monitor",
    author: "Linus Leo Stöckli",
};
const PREFERENCES_KEY: &str = "config/gui";
const PREFERENCES_KEY_SERIAL: &str = "config/serial_devices";

fn split(payload: &str) -> (Option<String>, Vec<f32>) {
    let mut split_data: Vec<&str> = vec![];
    for s in payload.split(':') {
        split_data.extend(s.split(','));
    }
    if split_data.is_empty() {
        return (None, vec![]);
    }
    // Try to parse the first entry as a number
    let first_entry = split_data[0];
    if first_entry.parse::<f32>().is_ok() {
        // First entry is a number → No identifier, process normally
        let values: Vec<f32> = split_data
            .iter()
            .map(|x| x.trim())
            .flat_map(|x| x.parse::<f32>())
            .collect();
        (None, values)
    } else {
        // First entry is a string identifier → Process with identifier
        let identifier = first_entry.to_string();
        let values: Vec<f32> = split_data[1..]
            .iter()
            .flat_map(|x| x.parse::<f32>())
            .collect();
        (Some(identifier), values)
    }
}

fn main_thread(
    sync_tx: Sender<bool>,
    data_lock: Arc<RwLock<DataContainer>>,
    raw_data_rx: Receiver<Packet>,
    save_rx: Receiver<FileOptions>,
    load_rx: Receiver<PathBuf>,
    load_names_tx: Sender<Vec<String>>,
    clear_rx: Receiver<bool>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataContainer::default();
    let mut identifier_map: HashMap<String, usize> = HashMap::new();
    let mut failed_format_counter = 0;

    let mut file_opened = false;

    loop {
        if let Ok(cl) = clear_rx.try_recv() {
            if cl {
                data = DataContainer::default();
                identifier_map = HashMap::new();
                failed_format_counter = 0;
            }
        }
        if !file_opened {
            if let Ok(packet) = raw_data_rx.try_recv() {
                data.loaded_from_file = false;
                if !packet.payload.is_empty() {
                    sync_tx.send(true).expect("unable to send sync tx");
                    data.raw_traffic.push(packet.clone());
                    data.absolute_time.push(packet.absolute_time);

                    if packet.direction == SerialDirection::Send {
                        continue;
                    }

                    let (identifier_opt, values) = split(&packet.payload);

                    if data.dataset.is_empty() || failed_format_counter > 10 {
                        // Reset dataset
                        data.dataset = vec![vec![]; values.len()];
                        failed_format_counter = 0;
                    }

                    if let Some(identifier) = identifier_opt {
                        let index =
                            *identifier_map.entry(identifier.clone()).or_insert_with(|| {
                                let new_index = data.dataset.len();
                                for _ in 0..values.len() {
                                    data.dataset.push(vec![]); // Ensure space for new identifier
                                    data.time.push(vec![]); // Ensure time tracking for this identifier
                                }
                                new_index
                            });

                        // // Ensure dataset and time vectors have enough columns
                        // while data.dataset.len() <= index {
                        //     data.dataset.push(vec![]);
                        //     data.time.push(vec![]);
                        // }

                        // Append values to corresponding dataset entries
                        for (i, &value) in values.iter().enumerate() {
                            data.dataset[index + i].push(value);
                            data.time[index + i].push(packet.relative_time);
                        }
                    } else {
                        // Handle unnamed datasets (pure numerical data)
                        if values.len() == data.dataset.len() {
                            for (i, &value) in values.iter().enumerate() {
                                data.dataset[i].push(value);
                                data.time[i].push(packet.relative_time);
                            }
                        } else {
                            failed_format_counter += 1;
                        }
                    }
                }
            }
        }
        if let Ok(fp) = load_rx.try_recv() {
            if let Some(file_ending) = fp.extension() {
                match file_ending.to_str().unwrap() {
                    "csv" => {
                        file_opened = true;
                        let mut file_options = FileOptions {
                            file_path: fp.clone(),
                            save_absolute_time: false,
                            save_raw_traffic: false,
                            names: vec![],
                        };
                        match open_from_csv(&mut data, &mut file_options) {
                            Ok(_) => {
                                log::info!("opened {:?}", fp);
                                load_names_tx
                                    .send(file_options.names)
                                    .expect("unable to send names on channel after loading");
                            }
                            Err(err) => {
                                file_opened = false;
                                log::error!("failed opening {:?}: {:?}", fp, err);
                            }
                        };
                    }
                    _ => {
                        file_opened = false;
                        log::error!("file not supported: {:?} \n Close the file to connect to a spectrometer or open another file.", fp);
                        continue;
                    }
                }
            } else {
                file_opened = false;
            }
        } else {
            file_opened = false;
        }

        if let Ok(mut write_guard) = data_lock.write() {
            *write_guard = data.clone();
        }

        if let Ok(csv_options) = save_rx.try_recv() {
            match save_to_csv(&data, &csv_options) {
                Ok(_) => {
                    log::info!("saved data file to {:?} ", csv_options.file_path);
                }
                Err(e) => {
                    log::error!(
                        "failed to save file to {:?}: {:?}",
                        csv_options.file_path,
                        e
                    );
                }
            }
        }
    }
}

fn main() {
    egui_logger::builder().init().unwrap();

    let gui_settings = load_gui_settings();
    let saved_serial_device_configs = load_serial_settings();

    let device_lock = Arc::new(RwLock::new(Device::default()));
    let devices_lock = Arc::new(RwLock::new(vec![gui_settings.device.clone()]));
    let data_lock = Arc::new(RwLock::new(DataContainer::default()));
    let connected_lock = Arc::new(RwLock::new(false));

    let (save_tx, save_rx): (Sender<FileOptions>, Receiver<FileOptions>) = mpsc::channel();
    let (load_tx, load_rx): (Sender<PathBuf>, Receiver<PathBuf>) = mpsc::channel();
    let (loaded_names_tx, loaded_names_rx): (Sender<Vec<String>>, Receiver<Vec<String>>) =
        mpsc::channel();
    let (send_tx, send_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (clear_tx, clear_rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let (raw_data_tx, raw_data_rx): (Sender<Packet>, Receiver<Packet>) = mpsc::channel();
    let (sync_tx, sync_rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

    let serial_device_lock = device_lock.clone();
    let serial_devices_lock = devices_lock.clone();
    let serial_connected_lock = connected_lock.clone();

    let _serial_thread_handler = thread::spawn(|| {
        serial_thread(
            send_rx,
            raw_data_tx,
            serial_device_lock,
            serial_devices_lock,
            serial_connected_lock,
        );
    });

    let main_data_lock = data_lock.clone();

    let _main_thread_handler = thread::spawn(|| {
        main_thread(
            sync_tx,
            main_data_lock,
            raw_data_rx,
            save_rx,
            load_rx,
            loaded_names_tx,
            clear_rx,
        );
    });

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        load_tx
            .send(PathBuf::from(&args[1]))
            .expect("failed to send file");
    }

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_inner_size(vec2(gui_settings.x, gui_settings.y))
            .with_min_inner_size(vec2(2.0 * RIGHT_PANEL_WIDTH, 2.0 * RIGHT_PANEL_WIDTH))
            .with_icon(
                icon_data::from_png_bytes(&include_bytes!("../icons/icon.png")[..]).unwrap(),
            ),
        ..Default::default()
    };

    let gui_data_lock = data_lock;
    let gui_device_lock = device_lock;
    let gui_devices_lock = devices_lock;
    let gui_connected_lock = connected_lock;

    if let Err(e) = eframe::run_native(
        "Serial Monitor",
        options,
        Box::new(|ctx| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            ctx.egui_ctx.set_fonts(fonts);
            ctx.egui_ctx.set_visuals(Visuals::dark());
            egui_extras::install_image_loaders(&ctx.egui_ctx);

            let repaint_signal = ctx.egui_ctx.clone();
            thread::spawn(move || loop {
                if sync_rx.recv().is_ok() {
                    repaint_signal.request_repaint();
                }
            });

            Ok(Box::new(MyApp::new(
                ctx,
                gui_data_lock,
                gui_device_lock,
                gui_devices_lock,
                saved_serial_device_configs,
                gui_connected_lock,
                gui_settings,
                save_tx,
                load_tx,
                loaded_names_rx,
                send_tx,
                clear_tx,
            )))
        }),
    ) {
        log::error!("{e:?}");
    }
}

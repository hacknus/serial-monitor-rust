#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use crate::data::{DataContainer, GuiOutputDataContainer, Packet, SerialDirection};
use crate::gui::{load_gui_settings, GuiCommand, MyApp, RIGHT_PANEL_WIDTH};
use crate::io::{open_from_csv, save_to_csv, FileOptions};
use crate::serial::{load_serial_settings, serial_thread, Device};
use crossbeam_channel::{select, Receiver, Sender};
use eframe::egui::{vec2, ViewportBuilder, Visuals};
use eframe::{egui, icon_data};
use egui_plot::PlotPoint;
use preferences::AppInfo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
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
            .filter_map(|x| match x.trim().parse::<f32>() {
                Ok(val) => Some(val),
                Err(_) => None,
            })
            .collect();
        (Some(identifier), values)
    }
}

fn console_text(show_timestamps: bool, show_sent_cmds: bool, packet: &Packet) -> Option<String> {
    match (show_sent_cmds, show_timestamps, &packet.direction) {
        (true, true, _) => Some(format!(
            "[{}] t + {:.3}s: {}\n",
            packet.direction,
            packet.relative_time as f32 / 1000.0,
            packet.payload
        )),
        (true, false, _) => Some(format!("[{}]: {}\n", packet.direction, packet.payload)),
        (false, true, SerialDirection::Receive) => Some(format!(
            "t + {:.3}s: {}\n",
            packet.relative_time as f32 / 1000.0,
            packet.payload
        )),
        (false, false, SerialDirection::Receive) => Some(packet.payload.clone() + "\n"),
        (_, _, _) => None,
    }
}

fn main_thread(
    sync_tx: Sender<bool>,
    data_lock: Arc<RwLock<GuiOutputDataContainer>>,
    raw_data_rx: Receiver<Packet>,
    save_rx: Receiver<FileOptions>,
    load_rx: Receiver<PathBuf>,
    load_names_tx: Sender<Vec<String>>,
    gui_cmd_rx: Receiver<GuiCommand>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataContainer::default();
    let mut identifier_map: HashMap<String, usize> = HashMap::new();
    let mut failed_format_counter = 0;
    let mut failed_key_counter = 0;

    let mut show_timestamps = true;
    let mut show_sent_cmds = true;

    let mut file_opened = false;

    loop {
        select! {
            recv(raw_data_rx) -> packet => {
                if let Ok(packet) = packet {
                    if !file_opened {
                        data.loaded_from_file = false;
                        if !packet.payload.is_empty() {
                            sync_tx.send(true).expect("unable to send sync tx");
                            data.raw_traffic.push(packet.clone());
                            data.absolute_time.push(packet.absolute_time);

                            if let Ok(mut gui_data) = data_lock.write() {
                                if let Some(text) = console_text(show_timestamps, show_sent_cmds, &packet) {
                                    // append prints
                                    gui_data.prints.push(text);
                                }
                            }

                            if packet.direction == SerialDirection::Receive {

                                let (identifier_opt, values) = split(&packet.payload);

                                if data.dataset.is_empty() || failed_format_counter > 10 {
                                    // resetting dataset
                                    data.time = vec![vec![]; values.len()];
                                    data.dataset = vec![vec![]; values.len()];
                                    if let Ok(mut gui_data) = data_lock.write() {
                                        gui_data.plots = (0..values.len())
                                            .map(|i| (format!("Column {i}"), vec![]))
                                            .collect();
                                    }
                                    failed_format_counter = 0;
                                    // log::error!("resetting dataset. split length = {}, length data.dataset = {}", split_data.len(), data.dataset.len());
                                }
                                // else if split_data.len() == data.dataset.len() {
                                //     // appending data
                                //     for (i, set) in data.dataset.iter_mut().enumerate() {
                                //         set.push(split_data[i]);
                                //         failed_format_counter = 0;
                                //         identifier_map = HashMap::new();
                                //     }
                                //
                                //     data.time.push(packet.relative_time);
                                //     data.absolute_time.push(packet.absolute_time);
                                //
                                //     // appending data for GUI thread
                                //     if let Ok(mut gui_data) = data_lock.write() {
                                //         // append plot-points
                                //         for ((_label, graph), data_i) in
                                //             gui_data.plots.iter_mut().zip(&data.dataset)
                                //         {
                                //             if data.time.len() == data_i.len() {
                                //                 if let Some(y) = data_i.last() {
                                //                     graph.push(PlotPoint {
                                //                         x: packet.relative_time / 1000.0,
                                //                         y: *y as f64,
                                //                     });
                                //                 }
                                //             }
                                //         }
                                //     }
                                //     if data.time.len() != data.dataset[0].len() {
                                //         // resetting dataset
                                //         data.time = vec![];
                                //         data.dataset = vec![vec![]; max(split_data.len(), 1)];
                                //         if let Ok(mut gui_data) = data_lock.write() {
                                //             gui_data.prints = vec!["".to_string(); max(split_data.len(), 1)];
                                //             gui_data.plots = (0..max(split_data.len(), 1))
                                //                 .map(|i| (format!("Column {i}"), vec![]))
                                //                 .collect();
                                //         }
                                //     }
                                // } else {
                                //     // not same length
                                //     failed_format_counter += 1;
                                //     // log::error!("not same length in main! length split_data = {}, length data.dataset = {}", split_data.len(), data.dataset.len())
                                // }

                                if let Some(identifier) = identifier_opt {
                                    if !identifier_map.contains_key(&identifier) {
                                        failed_key_counter += 1;
                                        if failed_key_counter < 10 && !identifier_map.is_empty() {
                                            continue; // skip outer loop iteration
                                        }

                                        let new_index = data.dataset.len();
                                        for _ in 0..values.len() {
                                            data.dataset.push(vec![]);
                                            data.time.push(vec![]);
                                        }

                                        if let Ok(mut gui_data) = data_lock.write() {
                                            gui_data.plots = (0..data.dataset.len())
                                                .map(|i| (format!("Column {i}"), vec![]))
                                                .collect();
                                        }

                                        identifier_map.insert(identifier.clone(), new_index);
                                    } else {
                                        failed_key_counter = 0;
                                    }

                                    let index = identifier_map[&identifier];

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

                                    if let Ok(mut gui_data) = data_lock.write() {
                                        for( ((_label, graph), data_i), time_i) in
                                            gui_data.plots.iter_mut().zip(&data.dataset).zip(&data.time)
                                        {
                                            if let (Some(y), Some(t)) = (data_i.last(), time_i.last() ){
                                                graph.push(PlotPoint {
                                                    x: *t / 1000.0,
                                                    y: *y as f64,
                                                });
                                            }

                                        }
                                    }
                                } else {
                                    // Handle unnamed datasets (pure numerical data)
                                    if values.len() == data.dataset.len() {
                                        for (i, &value) in values.iter().enumerate() {
                                            data.dataset[i].push(value);
                                            data.time[i].push(packet.relative_time);
                                        }
                                        if let Ok(mut gui_data) = data_lock.write() {
                                            for ((_label, graph), data_i) in
                                                gui_data.plots.iter_mut().zip(&data.dataset)
                                            {
                                                if data.time.len() == data_i.len() {
                                                    if let Some(y) = data_i.last() {
                                                        graph.push(PlotPoint {
                                                            x: packet.relative_time / 1000.0,
                                                            y: *y as f64,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        failed_format_counter += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            recv(gui_cmd_rx) -> msg => {
                if let Ok(cmd) = msg {
                    match cmd {
                        GuiCommand::Clear => {
                            data = DataContainer::default();
                            failed_format_counter = 0;
                            identifier_map = HashMap::new();
                            if let Ok(mut gui_data) = data_lock.write() {
                                *gui_data = GuiOutputDataContainer::default();
                            }
                        }
                        GuiCommand::ShowTimestamps(val) => {
                            show_timestamps = val;
                        }
                        GuiCommand::ShowSentTraffic(val) => {
                            show_sent_cmds = val;
                        }
                    }
                }
            }
            recv(load_rx) -> msg => {
                if let Ok(fp) = msg {
                    // load logic
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
                                    Ok(raw_data) => {
                                        log::info!("opened {:?}", fp);
                                        if let Ok(mut gui_data) = data_lock.write() {

                                            gui_data.prints = raw_data;

                                            dbg!(&gui_data.prints);

                                            gui_data.plots = (0..data.dataset.len())
                                                .map(|i| (file_options.names[i].to_string(), vec![]))
                                                .collect();
                                            // append plot-points
                                            for ((_label, graph), data_i) in
                                                gui_data.plots.iter_mut().zip(&data.dataset)
                                            {
                                                for (y,t) in data_i.iter().zip(data.time.iter()) {
                                                        graph.push(PlotPoint {
                                                            // TODO: this always takes the first time value
                                                            x: t[0] / 1000.0,
                                                            y: *y as f64 ,
                                                        });
                                                }
                                            }

                                        }
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
            }
            recv(save_rx) -> msg => {
                if let Ok(csv_options) = msg {
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
            default(Duration::from_millis(10)) => {
                // occasionally push data to GUI
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
    let data_lock = Arc::new(RwLock::new(GuiOutputDataContainer::default()));
    let connected_lock = Arc::new(RwLock::new(false));

    let (save_tx, save_rx): (Sender<FileOptions>, Receiver<FileOptions>) =
        crossbeam_channel::unbounded();
    let (load_tx, load_rx): (Sender<PathBuf>, Receiver<PathBuf>) = crossbeam_channel::unbounded();
    let (loaded_names_tx, loaded_names_rx): (Sender<Vec<String>>, Receiver<Vec<String>>) =
        crossbeam_channel::unbounded();
    let (send_tx, send_rx): (Sender<String>, Receiver<String>) = crossbeam_channel::unbounded();
    let (gui_cmd_tx, gui_cmd_rx): (Sender<GuiCommand>, Receiver<GuiCommand>) =
        crossbeam_channel::unbounded();
    let (raw_data_tx, raw_data_rx): (Sender<Packet>, Receiver<Packet>) =
        crossbeam_channel::unbounded();
    let (sync_tx, sync_rx): (Sender<bool>, Receiver<bool>) = crossbeam_channel::unbounded();

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
            gui_cmd_rx,
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
                gui_cmd_tx,
            )))
        }),
    ) {
        log::error!("{e:?}");
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use crate::data::{DataContainer, GuiOutputDataContainer, Packet, SerialDirection};
use crate::gui::{load_gui_settings, GuiCommand, MyApp, RIGHT_PANEL_WIDTH};
use crate::io::{open_from_csv, save_to_csv, FileOptions};
use crate::serial::{load_serial_settings, serial_devices_thread, serial_thread, Device};
use crossbeam_channel::{select, Receiver, Sender};
use eframe::egui::{vec2, ViewportBuilder};
use eframe::{egui, icon_data};
use egui_plot::PlotPoint;
pub use gumdrop::Options;
use preferences::AppInfo;
use std::cmp::max;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

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
    cli_column_labels: Vec<String>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataContainer::default();
    let mut failed_format_counter = 0;

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

                            if let Some(text) = console_text(show_timestamps, show_sent_cmds, &packet) {
                                // append prints
                                if let Ok(mut gui_data) = data_lock.write() {
                                    gui_data.prints.push(text);
                                }
                            }

                            let split_data = split(&packet.payload);
                            if data.dataset.is_empty() || failed_format_counter > 10 {
                                // resetting dataset
                                data.time = vec![];
                                data.dataset = vec![vec![]; max(split_data.len(), 1)];
                                if let Ok(mut gui_data) = data_lock.write() {
                                    gui_data.plots = (0..max(split_data.len(), 1))
                                        .map(|i| (cli_column_labels.get(i).cloned().unwrap_or_else(|| format!("Column {i}")), vec![]))
                                        .collect();
                                }
                                failed_format_counter = 0;
                                // log::error!("resetting dataset. split length = {}, length data.dataset = {}", split_data.len(), data.dataset.len());
                            } else if split_data.len() == data.dataset.len() {
                                // appending data
                                for (i, set) in data.dataset.iter_mut().enumerate() {
                                    set.push(split_data[i]);
                                    failed_format_counter = 0;
                                }

                                data.time.push(packet.relative_time);
                                data.absolute_time.push(packet.absolute_time);

                                // appending data for GUI thread
                                if let Ok(mut gui_data) = data_lock.write() {
                                    // append plot-points
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
                                if data.time.len() != data.dataset[0].len() {
                                    // resetting dataset
                                    data.time = vec![];
                                    data.dataset = vec![vec![]; max(split_data.len(), 1)];
                                    if let Ok(mut gui_data) = data_lock.write() {
                                        gui_data.prints = vec!["".to_string(); max(split_data.len(), 1)];
                                        gui_data.plots = (0..max(split_data.len(), 1))
                                            .map(|i| (format!("Column {i}"), vec![]))
                                            .collect();
                                    }
                                }
                            } else {
                                // not same length
                                failed_format_counter += 1;
                                // log::error!("not same length in main! length split_data = {}, length data.dataset = {}", split_data.len(), data.dataset.len())
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

                                            gui_data.plots = (0..data.dataset.len())
                                                .map(|i| (file_options.names[i].to_string(), vec![]))
                                                .collect();
                                            // append plot-points
                                            for ((_label, graph), data_i) in
                                                gui_data.plots.iter_mut().zip(&data.dataset)
                                            {
                                                for (y,t) in data_i.iter().zip(data.time.iter()) {
                                                        graph.push(PlotPoint {
                                                            x: *t / 1000.0,
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

fn parse_databits(s: &str) -> Result<serialport::DataBits, String> {
    let d: u8 = s
        .parse()
        .map_err(|_e| format!("databits not a number: {s}"))?;
    Ok(serialport::DataBits::try_from(d).map_err(|_e| format!("invalid databits: {s}"))?)
}

fn parse_flow(s: &str) -> Result<serialport::FlowControl, String> {
    match s {
        "none" => Ok(serialport::FlowControl::None),
        "soft" => Ok(serialport::FlowControl::Software),
        "hard" => Ok(serialport::FlowControl::Hardware),
        _ => Err(format!("invalid flow-control: {s}")),
    }
}

fn parse_stopbits(s: &str) -> Result<serialport::StopBits, String> {
    let d: u8 = s
        .parse()
        .map_err(|_e| format!("stopbits not a number: {s}"))?;
    Ok(serialport::StopBits::try_from(d).map_err(|_e| format!("invalid stopbits: {s}"))?)
}

fn parse_parity(s: &str) -> Result<serialport::Parity, String> {
    match s {
        "none" => Ok(serialport::Parity::None),
        "odd" => Ok(serialport::Parity::Odd),
        "even" => Ok(serialport::Parity::Even),
        _ => Err(format!("invalid parity setting: {s}")),
    }
}

fn parse_color(s: &str) -> Result<egui::Color32, String> {
    Ok(egui::ecolor::HexColor::from_str_without_hash(s)
        .map_err(|e| format!("invalid color {s:?}: {e:?}"))?
        .color())
}

#[derive(Debug, Options)]
struct CliOptions {
    /// Serial port device to open on startup
    #[options(free)]
    device: Option<String>,

    /// Baudrate (default=9600)
    #[options(short = "b")]
    baudrate: Option<u32>,

    /// Data bits (5, 6, 7, default=8)
    #[options(short = "d", parse(try_from_str = "parse_databits"))]
    databits: Option<serialport::DataBits>,

    /// Flow conrol (hard, soft, default=none)
    #[options(short = "f", parse(try_from_str = "parse_flow"))]
    flow: Option<serialport::FlowControl>,

    /// Stop bits (default=1, 2)
    #[options(short = "s", parse(try_from_str = "parse_stopbits"))]
    stopbits: Option<serialport::StopBits>,

    /// Parity (odd, even, default=none)
    #[options(short = "p", parse(try_from_str = "parse_parity"))]
    parity: Option<serialport::Parity>,

    /// Load data from a file instead of a serial port
    #[options(short = "F")]
    file: Option<std::path::PathBuf>,

    /// Column labels, can be specified multiple times for more columns
    #[options(no_short, long = "column")]
    column_labels: Vec<String>,

    /// Column colors (hex color without #), can be specified multiple times for more columns
    #[options(no_short, long = "color", parse(try_from_str = "parse_color"))]
    column_colors: Vec<egui::Color32>,

    help: bool,
}

fn main() {
    egui_logger::builder().init().unwrap();

    let args = CliOptions::parse_args_default_or_exit();

    let gui_settings = load_gui_settings();
    let saved_serial_device_configs = load_serial_settings();

    let mut device = Device::default();
    if let Some(name) = args.device {
        device.name = name;
    }
    if let Some(baudrate) = args.baudrate {
        device.baud_rate = baudrate;
    }
    if let Some(databits) = args.databits {
        device.data_bits = databits;
    }
    if let Some(flow) = args.flow {
        device.flow_control = flow;
    }
    if let Some(stopbits) = args.stopbits {
        device.stop_bits = stopbits;
    }
    if let Some(parity) = args.parity {
        device.parity = parity;
    }

    let device_lock = Arc::new(RwLock::new(device));
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

    let serial_2_devices_lock = devices_lock.clone();

    let _serial_devices_thread_handler = thread::spawn(|| {
        serial_devices_thread(serial_2_devices_lock);
    });

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
            args.column_labels,
        );
    });

    if let Some(file) = args.file {
        load_tx.send(file).expect("failed to send file");
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
            ctx.egui_ctx.set_theme(gui_settings.theme_preference);
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
                args.column_colors,
            )))
        }),
    ) {
        log::error!("{e:?}");
    }
}

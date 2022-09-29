use core::f32;
use std::ops::RangeInclusive;
use std::sync::mpsc::{Sender};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::fs;
use eframe::{egui, Storage};
use eframe::egui::panel::{Side};
use eframe::egui::plot::{Legend, Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use eframe::egui::{Checkbox, FontId, FontFamily, RichText, Stroke, global_dark_light_mode_buttons};
use crate::toggle::toggle;
use egui_extras::RetainedImage;
use itertools_num::linspace;
use preferences::Preferences;
use crate::{APP_INFO, vec2};
use serde::{Deserialize, Serialize};
use crate::data::{DataContainer};


const MAX_FPS: f64 = 24.0;


#[derive(Clone)]
pub enum Print {
    EMPTY,
    MESSAGE(String),
    ERROR(String),
    DEBUG(String),
    TASK(String),
    OK(String),
}

pub fn print_to_console(print_lock: &Arc<RwLock<Vec<Print>>>, message: Print) -> usize {
    let mut length: usize = 0;
    if let Ok(mut write_guard) = print_lock.write() {
        write_guard.push(message);
        length = write_guard.len() - 1;
    }
    length
}

pub fn update_in_console(print_lock: &Arc<RwLock<Vec<Print>>>, message: Print, index: usize) {
    if let Ok(mut write_guard) = print_lock.write() {
        write_guard[index] = message;
    }
}

pub enum GuiState {
    IDLE,
    Heater1Temperature(f32),
    Heater2Temperature(f32),
    Pump(bool),
    Heater1(bool),
    Heater2(bool),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GuiSettingsContainer {
    pub device: String,
    pub baud: u32,
    pub debug: bool,
    pub x: f32,
    pub y: f32,
}

impl GuiSettingsContainer {
    pub fn default() -> GuiSettingsContainer {
        return GuiSettingsContainer {
            device: "".to_string(),
            baud: 115_200,
            debug: true,
            x: 1600.0,
            y: 900.0,
        };
    }
}

pub struct MyApp {
    dark_mode: bool,
    ready: bool,
    command: String,
    device: String,
    baud_rate: u32,
    plotting_range: f32,
    console: Vec<Print>,
    graph_visible: Vec<bool>,
    dropped_files: Vec<egui::DroppedFile>,
    picked_path: String,
    data: DataContainer,
    gui_conf: GuiSettingsContainer,
    print_lock: Arc<RwLock<Vec<Print>>>,
    device_lock: Arc<RwLock<String>>,
    devices_lock: Arc<RwLock<Vec<String>>>,
    baud_lock: Arc<RwLock<u32>>,
    connected_lock: Arc<RwLock<bool>>,
    data_lock: Arc<RwLock<DataContainer>>,
    config_tx: Sender<Vec<GuiState>>,
    save_tx: Sender<String>,
    send_tx: Sender<String>,
    eol: String,
    show_sent_cmds: bool,
    show_timestamps: bool,
    save_raw: bool,
}

impl MyApp {
    pub fn new(print_lock: Arc<RwLock<Vec<Print>>>,
               data_lock: Arc<RwLock<DataContainer>>,
               device_lock: Arc<RwLock<String>>,
               devices_lock: Arc<RwLock<Vec<String>>>,
               baud_lock: Arc<RwLock<u32>>,
               connected_lock: Arc<RwLock<bool>>,
               gui_conf: GuiSettingsContainer,
               config_tx: Sender<Vec<GuiState>>,
               save_tx: Sender<String>,
               send_tx: Sender<String>,
    ) -> Self {
        Self {
            dark_mode: true,
            ready: false,
            dropped_files: vec![],
            picked_path: "".to_string(),
            device: "".to_string(),
            data: DataContainer::default(),
            console: vec![Print::MESSAGE(format!("waiting for serial connection..,").to_string())],
            connected_lock,
            device_lock,
            devices_lock,
            baud_lock,
            print_lock,
            gui_conf,
            data_lock,
            config_tx,
            save_tx,
            send_tx,
            plotting_range: 100.0,
            command: "".to_string(),
            graph_visible: vec![],
            baud_rate: 9600,
            show_sent_cmds: true,
            show_timestamps: true,
            save_raw: true,
            eol: "\r\n".to_string(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut gui_states: Vec<GuiState> = vec![];

        if let Ok(read_guard) = self.connected_lock.read() {
            self.ready = read_guard.clone();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let height = ui.available_size().y * 0.45;
            let spacing = (ui.available_size().y - 2.0 * height) / 3.0 - 10.0;
            let width = ui.available_size().x * 0.8;
            ui.add_space(spacing);

            if let Ok(read_guard) = self.data_lock.read() {
                self.data = read_guard.clone();
                // self.data.time = linspace::<f32>(self.tera_flash_conf.t_begin as f32,
                //                                  (self.tera_flash_conf.t_begin + self.tera_flash_conf.range) as f32, 1000).collect();
            }

            let mut graphs: Vec<Vec<[f64; 2]>> = vec![vec![]; self.data.dataset.len()];
            for i in 0..self.data.dataset[0].len() {
                for (graph, data) in graphs.iter_mut().zip(&self.data.dataset) {
                    //graph.push([i as f64, data[i] as f64]);
                    if self.data.time.len() == data.len() {
                        graph.push([self.data.time[i] as f64, data[i] as f64]);
                    } else {
                        // not same length
                        //println!("not same length in gui! length self.data.time = {}, length data = {}", self.data.time.len(), data.len())
                    }
                }
            }

            let t_fmt = |x, _range: &RangeInclusive<f64>| {
                format!("{:4.2} s", x)
            };
            let s_fmt = move |y, _range: &RangeInclusive<f64>| {
                format!("{:4.2} [a.u.]", y as f64)
            };
            let signal_plot = Plot::new("data")
                .height(height)
                .width(width)
                .legend(Legend::default())
                .y_axis_formatter(s_fmt)
                .x_axis_formatter(t_fmt)
                .min_size(vec2(50.0, 100.0));


            signal_plot.show(ui, |signal_plot_ui| {
                for (i, graph) in graphs.iter().enumerate() {
                    signal_plot_ui.line(Line::new(PlotPoints::from(graph.clone()))
                        .name(format!("Column {}", i)));
                }
            });

            let num_rows = self.data.raw_traffic.len();
            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            ui.add_space(spacing);

            ui.separator();
            egui::ScrollArea::vertical()
                .id_source("serial_output")
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .always_show_scroll(true)
                .enable_scrolling(true)
                .max_height(height)
                .max_width(width)
                .show_rows(ui, row_height, num_rows,
                           |ui, row_range| {
                               for row in row_range {
                                   let packet = self.data.raw_traffic[row].clone();
                                   let color;
                                   if self.dark_mode {
                                       color = egui::Color32::WHITE;
                                   } else {
                                       color = egui::Color32::BLACK;
                                   }
                                   ui.horizontal_wrapped(|ui| {
                                       let text = format!("[{}] {:3}: {}",
                                                          packet.direction,
                                                          packet.time,
                                                          packet.payload);
                                       ui.label(RichText::new(text).color(color).font(
                                           FontId::new(14.0, FontFamily::Monospace)));
                                   });
                               }
                           });
            let mut text_triggered = false;
            let mut button_triggered = false;
            ui.horizontal(|ui| {
                text_triggered = ui.add(egui::TextEdit::singleline(&mut self.command).desired_width(width)).lost_focus();
                button_triggered = ui.button("Send").clicked();
            });
            if text_triggered || button_triggered {
                // send command
                self.send_tx.send(self.command.clone() + &self.eol.clone());
            }
            ctx.request_repaint()
        });

        egui::SidePanel::new(Side::Right, 3)
            .min_width(100.0)
            .show(ctx, |ui| {
                ui.add_enabled_ui(true, |ui| {
                    ui.set_visible(true);
                    ui.horizontal(|ui| {
                        ui.heading("Serial Monitor");
                        // TODO: only run this when the system is waiting for a response
                        if !self.ready {
                            ui.add(egui::Spinner::new());
                        }
                        let radius = &ui.spacing().interact_size.y * 0.375;
                        let center = egui::pos2(ui.next_widget_position().x + &ui.spacing().interact_size.x * 0.5, ui.next_widget_position().y);
                        ui.painter()
                            .circle(center, radius, egui::Color32::DARK_GREEN, egui::Stroke::new(1.0, egui::Color32::GREEN));
                    });

                    let mut devices: Vec<String> = Vec::new();
                    if let Ok(read_guard) = self.devices_lock.read() {
                        devices = read_guard.clone();
                    }

                    egui::ComboBox::from_id_source("Device")
                        .selected_text(&self.device)
                        .show_ui(ui, |ui| {
                            for dev in devices {
                                ui.selectable_value(&mut self.device, dev.clone(), dev);
                            }
                        });
                    egui::ComboBox::from_id_source("Baud Rate")
                        .selected_text(&format!("{}", self.baud_rate))
                        .show_ui(ui, |ui| {
                            let baud_rates = vec![
                                300, 1200, 2400, 4800, 9600, 19200,
                                38400, 57600, 74880, 115200, 230400, 128000,
                                460800, 576000, 921600,
                            ];
                            for baud_rate in baud_rates.iter() {
                                ui.selectable_value(
                                    &mut self.baud_rate,
                                    baud_rate.clone(),
                                    format!("{}", baud_rate),
                                );
                            }
                        });

                    let connect_text: &str;
                    if self.ready {
                        connect_text = "Disconnect";
                    } else {
                        connect_text = "Connect";
                    }
                    if ui.button(connect_text).clicked() {
                        if let Ok(mut write_guard) = self.device_lock.write() {
                            if self.ready {
                                *write_guard = "".to_string();
                                self.device = "".to_string();
                            } else {
                                *write_guard = self.device.clone();
                            }
                        }
                        if let Ok(mut write_guard) = self.baud_lock.write() {
                            if self.ready {
                                *write_guard = 0;
                                self.baud_rate = 0;
                            } else {
                                *write_guard = self.baud_rate.clone();
                            }
                        }
                    }

                    egui::Grid::new("upper")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Plotting range [s]: ");
                            if ui.add(egui::DragValue::new(&mut self.plotting_range)).lost_focus() {
                                //gui_states.push(GuiState::TBegin(self.tera_flash_conf.t_begin));
                            };
                            ui.end_row();
                            if ui.button("Save to file").clicked() {
                                match rfd::FileDialog::new().save_file() {
                                    Some(path) =>
                                    // TODO: here we should really include .csv as extension!
                                        {
                                            let extension = ".csv".to_string();
                                            let mut final_path: String;
                                            if path.display().to_string().ends_with(".csv") {
                                                final_path = path.display().to_string();
                                            } else {
                                                final_path = path.display().to_string();
                                                final_path.push_str(&extension);
                                            }
                                            self.picked_path = final_path;
                                        }
                                    None => self.picked_path = "".to_string()
                                }
                                self.save_tx.send(self.picked_path.clone());
                            }
                            ui.end_row();
                            ui.label("Save Raw Traffic");
                            if ui.add(toggle(&mut self.save_raw)).changed() {
                                // gui_states.push(GuiState::Run(self.show_timestamps));
                            }
                            ui.end_row();
                            ui.label("");
                            ui.end_row();
                            ui.label("Show Sent Commands");
                            if ui.add(toggle(&mut self.show_sent_cmds)).changed() {
                                // gui_states.push(GuiState::Run(self.show_sent_cmds));
                            }
                            ui.end_row();
                            ui.label("Show Timestamp");
                            if ui.add(toggle(&mut self.show_timestamps)).changed() {
                                // gui_states.push(GuiState::Run(self.show_timestamps));
                            }
                            ui.end_row();
                            ui.label("EOL character");
                            ui.text_edit_singleline(&mut self.eol);
                            // ui.checkbox(&mut self.gui_conf.debug, "Debug Mode");
                            ui.end_row();
                            global_dark_light_mode_buttons(ui);
                            ui.end_row();
                            ui.label("");
                            ui.end_row();
                        });
                });
                let num_rows = self.console.len();
                let text_style = egui::TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                egui::ScrollArea::vertical()
                    .id_source("console_scroll_area")
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .max_height(row_height * 5.0)
                    .show_rows(ui, row_height, num_rows,
                               |ui, row_range| {
                                   for row in row_range {
                                       match self.console[row].clone() {
                                           Print::EMPTY => {}
                                           Print::MESSAGE(s) => {
                                               let text = "[MSG] ".to_string();
                                               ui.horizontal_wrapped(|ui| {
                                                   let color: egui::Color32;
                                                   if self.dark_mode {
                                                       color = egui::Color32::WHITE;
                                                   } else {
                                                       color = egui::Color32::BLACK;
                                                   }
                                                   ui.label(RichText::new(text).color(color).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                           Print::ERROR(s) => {
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[ERR] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::RED).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                           Print::DEBUG(s) => {
                                               if self.gui_conf.debug {
                                                   let color: egui::Color32;
                                                   if self.dark_mode {
                                                       color = egui::Color32::YELLOW;
                                                   } else {
                                                       color = egui::Color32::LIGHT_RED;
                                                   }
                                                   ui.horizontal_wrapped(|ui| {
                                                       let text = "[DBG] ".to_string();
                                                       ui.label(RichText::new(text).color(color).font(
                                                           FontId::new(14.0, FontFamily::Monospace)));
                                                       let text = format!("{}", s);
                                                       ui.label(RichText::new(text).font(
                                                           FontId::new(14.0, FontFamily::Monospace)));
                                                   });
                                               }
                                           }
                                           Print::TASK(s) => {
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[  ] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::WHITE).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                           Print::OK(s) => {
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[OK] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::GREEN).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                       }
                                   }
                               });
            });

        self.gui_conf.x = ctx.used_size().x;
        self.gui_conf.y = ctx.used_size().y;

        if !gui_states.is_empty() {
            self.config_tx.send(gui_states);
        }
        std::thread::sleep(Duration::from_millis((1000.0 / MAX_FPS) as u64));
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        let prefs_key = "config/gui";
        self.gui_conf.save(&APP_INFO, prefs_key);
    }
}
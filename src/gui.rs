use core::f32;
use std::ops::RangeInclusive;
use std::sync::mpsc::{Sender};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::fs;
use eframe::{egui, Storage};
use eframe::egui::panel::{Side};
use eframe::egui::plot::{Line, LineStyle, Plot, Value, Values, VLine};
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
    pub debug: bool,
    pub x: f32,
    pub y: f32,
}

impl GuiSettingsContainer {
    pub fn default() -> GuiSettingsContainer {
        return GuiSettingsContainer {
            device : "".to_string(),
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
    console: Vec<Print>,
    graph_visible: Vec<bool>,
    dropped_files: Vec<egui::DroppedFile>,
    picked_path: String,
    data: DataContainer,
    gui_conf: GuiSettingsContainer,
    print_lock: Arc<RwLock<Vec<Print>>>,
    device_lock: Arc<RwLock<String>>,
    connected_lock: Arc<RwLock<bool>>,
    data_lock: Arc<RwLock<DataContainer>>,
    config_tx: Sender<Vec<GuiState>>,
    save_tx: Sender<String>,
}

impl MyApp {
    pub fn new(print_lock: Arc<RwLock<Vec<Print>>>,
               data_lock: Arc<RwLock<DataContainer>>,
               device_lock: Arc<RwLock<String>>,
               connected_lock: Arc<RwLock<bool>>,
               gui_conf: GuiSettingsContainer,
               config_tx: Sender<Vec<GuiState>>,
               save_tx: Sender<String>,
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
            print_lock,
            gui_conf,
            data_lock,
            config_tx,
            save_tx,
            command: "".to_string(),
            graph_visible: vec![]
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut gui_states: Vec<GuiState> = vec![];

        egui::CentralPanel::default().show(ctx, |ui| {
            let height = ui.available_size().y * 0.9;
            let spacing = (ui.available_size().y - height) / 2.0 - 10.0;
            let width = ui.available_size().x * 0.8;
            ui.add_space(spacing);

            ui.horizontal(|ui| {
                let mut graph_counter = 0;
                for vis in self.graph_visible.iter_mut(){
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(vis, ""));
                    ui.colored_label(egui::Color32::RED, "— ");
                    ui.label(format!("{}",graph_counter));
                }
            });

            if let Ok(read_guard) = self.data_lock.read() {
                self.data = read_guard.clone();
                // self.data.time = linspace::<f32>(self.tera_flash_conf.t_begin as f32,
                //                                  (self.tera_flash_conf.t_begin + self.tera_flash_conf.range) as f32, 1000).collect();
            }

            let mut graphs: Vec<Vec<Value>> = vec![vec![]];

            for i in 0..self.data.time.len() {
                for (graph,data) in graphs.iter_mut().zip(&self.data.dataset){
                    graph.push(Value { x: self.data.time[i] as f64, y: data[i] as f64 });
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
                .y_axis_formatter(s_fmt)
                .x_axis_formatter(t_fmt)
                //.include_x(&self.tera_flash_conf.t_begin + &self.tera_flash_conf.range)
                //.include_x(self.tera_flash_conf.t_begin)
                .min_size(vec2(50.0, 100.0));


            signal_plot.show(ui, |signal_plot_ui| {
                for (i,(vis,graph)) in self.graph_visible.iter().zip(graphs).enumerate(){
                    if *vis {
                        signal_plot_ui.line(Line::new(Values::from_values(graph))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .name(format!("{}",i)));
                    }
                }

            });
            ctx.request_repaint()
        });

        egui::SidePanel::new(Side::Right, 3)
            .min_width(200.0)
            .show(ctx, |ui| {
            ui.add_enabled_ui(true, |ui| {
                ui.set_visible(true);
                ui.horizontal(|ui| {
                    ui.heading("Serial Monitor");
                    // TODO: only run this when the system is waiting for a response
                    ui.add(egui::Spinner::new());
                    let radius = &ui.spacing().interact_size.y * 0.375;
                    let center = egui::pos2(ui.next_widget_position().x + &ui.spacing().interact_size.x * 0.5, ui.next_widget_position().y);
                    ui.painter()
                        .circle(center, radius, egui::Color32::DARK_GREEN, egui::Stroke::new(1.0, egui::Color32::GREEN));
                });

                let mut devices: Vec<String> = Vec::new();
                match serialport::available_ports() {
                    Ok(ports) => {
                        // maybe remove bluetooth port here...
                        for port in ports {
                            devices.push(port.port_name);
                        }
                    }
                    Err(e) => {
                        devices.push("no devices found".to_string());
                    }
                }

                egui::ComboBox::from_id_source("Device")
                    .selected_text(&self.device)
                    .show_ui(ui, |ui| {
                        for dev in devices {
                            ui.selectable_value(&mut self.device, dev.clone(), dev);
                        }
                    },
                    );
                // gui_states.push(GuiState::Channel(self.tera_flash_conf.channel.clone()));

                egui::Grid::new("upper")
                    .num_columns(2)
                    .spacing([80.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {

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
                        ui.label("");
                        ui.end_row();
                        ui.checkbox(&mut self.gui_conf.debug, "Debug Mode");
                        ui.end_row();
                        global_dark_light_mode_buttons(ui);
                        ui.end_row();
                        ui.label("");
                        ui.end_row();
                    });
                let num_rows = self.console.len();
                let text_style = egui::TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_source("console_scroll_area")
                    .auto_shrink([false; 2])
                    .stick_to_bottom()
                    .max_height(row_height * 5.20)
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
                ui.text_edit_singleline(&mut self.command);
                ui.add_space(5.0);
                let height = ui.available_size().y;
                ui.add_space(height);
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
use core::f32;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui::{
    global_dark_light_mode_buttons, Align2, FontFamily, FontId, KeyboardShortcut, Pos2, Sense,
    SidePanel, Vec2, Visuals,
};
use eframe::{egui, Storage};
use egui_plot::{log_grid_spacer, Legend, Line, Plot, PlotPoint, PlotPoints};
use preferences::Preferences;
use serde::{Deserialize, Serialize};
use serialport::{DataBits, FlowControl, Parity, StopBits};

use crate::data::{DataContainer, SerialDirection};
use crate::record::RecordOptions;
use crate::serial::{clear_serial_settings, save_serial_settings, Device, SerialDevices};
use crate::toggle::toggle;
use crate::{FileOptions, GuiEvent};
use crate::{APP_INFO, PREFS_KEY};

mod components;

const MAX_FPS: f64 = 60.0;

const DEFAULT_FONT_ID: FontId = FontId::new(14.0, FontFamily::Monospace);
pub const RIGHT_PANEL_WIDTH: f32 = 350.0;
const BAUD_RATES: &[u32] = &[
    300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 74880, 115200, 230400, 128000, 460800,
    576000, 921600,
];

const SAVE_FILE_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);

// bitOr is not const, so we use plus
const SAVE_PLOT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(
    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
    egui::Key::S,
);

const CLEAR_PLOT_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::X);

#[derive(Clone)]
#[allow(unused)]
pub enum Print {
    Empty,
    Message(String),
    Error(String),
    Debug(String),
    Ok(String),
}

#[derive(PartialEq)]
pub enum WindowFeedback {
    None,
    Waiting,
    Clear,
    Cancel,
}

impl Print {
    pub fn scroll_area_message(
        &self,
        gui_conf: &GuiSettingsContainer,
    ) -> Option<ScrollAreaMessage> {
        match self {
            Print::Empty => None,
            Print::Message(s) => {
                let color = if gui_conf.dark_mode {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::BLACK
                };
                Some(ScrollAreaMessage {
                    label: "[MSG] ".to_owned(),
                    content: s.to_owned(),
                    color,
                })
            }
            Print::Error(s) => {
                let color = egui::Color32::RED;
                Some(ScrollAreaMessage {
                    label: "[ERR] ".to_owned(),
                    content: s.to_owned(),
                    color,
                })
            }
            Print::Debug(s) => {
                let color = if gui_conf.dark_mode {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_RED
                };
                Some(ScrollAreaMessage {
                    label: "[DBG] ".to_owned(),
                    content: s.to_owned(),
                    color,
                })
            }
            Print::Ok(s) => {
                let color = egui::Color32::GREEN;
                Some(ScrollAreaMessage {
                    label: "[OK] ".to_owned(),
                    content: s.to_owned(),
                    color,
                })
            }
        }
    }
}

#[allow(dead_code)]
pub struct ScrollAreaMessage {
    label: String,
    content: String,
    color: egui::Color32,
}

pub fn print_to_console(print_lock: &Arc<RwLock<Vec<Print>>>, message: Print) {
    match print_lock.write() {
        Ok(mut write_guard) => {
            write_guard.push(message);
        }
        Err(e) => {
            println!("Error while writing to print_lock: {}", e);
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Command {
    name: String,
    cmd: String,
    editing: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GuiSettingsContainer {
    pub device: String,
    pub baud: u32,
    pub debug: bool,
    pub x: f32,
    pub y: f32,
    pub save_absolute_time: bool,
    pub dark_mode: bool,
    pub plot_options: PlotOptions,
    pub raw_traffic_options: RawTrafficOptions,
    pub record_options: RecordOptions,
    pub commands: Vec<Command>,
}

impl Default for GuiSettingsContainer {
    fn default() -> Self {
        Self {
            device: "".to_string(),
            baud: 115_200,
            debug: true,
            x: 1600.0,
            y: 900.0,
            save_absolute_time: false,
            dark_mode: true,
            plot_options: PlotOptions::default(),
            raw_traffic_options: RawTrafficOptions::default(),
            record_options: RecordOptions::default(),
            commands: vec![Command {
                name: "Command 1".to_owned(),
                cmd: "".to_owned(),
                editing: false,
            }],
        }
    }
}

pub fn load_gui_settings() -> GuiSettingsContainer {
    let mut gui_settings = GuiSettingsContainer::load(&APP_INFO, PREFS_KEY).unwrap_or_else(|_| {
        let gui_settings = GuiSettingsContainer::default();
        // save default settings
        if gui_settings.save(&APP_INFO, PREFS_KEY).is_err() {
            println!("failed to save gui_settings");
        }
        gui_settings
    });
    gui_settings.record_options.enable = false;
    gui_settings.record_options.record_path = PathBuf::new();
    gui_settings
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GuiTabs {
    RawTraffic,
    Commands,
    PlotOptions,
    Record,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTrafficOptions {
    pub enable: bool,
    show_sent_cmds: bool,
    show_timestamps: bool,
    pub max_len: usize,
    eol: String,
}

impl Default for RawTrafficOptions {
    fn default() -> Self {
        Self {
            enable: false,
            show_sent_cmds: true,
            show_timestamps: true,
            max_len: 5000,
            eol: "\\r\\n".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlotOptions {
    pub buffer_size: usize,
    plotting_range: usize,
    labels: Vec<String>,
    number_of_plots: usize,
    time_x_axis: bool,
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            buffer_size: 5000,
            plotting_range: usize::MAX,
            labels: vec!["Column 0".to_string()],
            number_of_plots: 1,
            time_x_axis: false,
        }
    }
}

pub struct MyApp {
    connected_to_device: bool,
    command: String,
    device: String,
    old_device: String,
    device_idx: usize,
    serial_devices: SerialDevices,
    plot_serial_display_ratio: f32,
    console: Vec<Print>,
    picked_path: PathBuf,
    plot_location: Option<egui::Rect>,
    data: DataContainer,
    gui_conf: GuiSettingsContainer,
    print_lock: Arc<RwLock<Vec<Print>>>,
    device_lock: Arc<RwLock<Device>>,
    devices_lock: Arc<RwLock<Vec<String>>>,
    connected_lock: Arc<RwLock<bool>>,
    data_lock: Arc<RwLock<DataContainer>>,
    send_tx: Sender<String>,
    gui_event_tx: Sender<GuiEvent>,
    record_options_tx: Sender<RecordOptions>,
    history: Vec<String>,
    index: usize,
    save_raw: bool,
    show_warning_window: WindowFeedback,
    do_not_show_clear_warning: bool,
    need_initialize: bool,
    right_panel_expanded: bool,
    active_tab: Option<GuiTabs>,
}

#[allow(clippy::too_many_arguments)]
impl MyApp {
    pub fn new(
        print_lock: Arc<RwLock<Vec<Print>>>,
        data_lock: Arc<RwLock<DataContainer>>,
        device_lock: Arc<RwLock<Device>>,
        devices_lock: Arc<RwLock<Vec<String>>>,
        devices: SerialDevices,
        connected_lock: Arc<RwLock<bool>>,
        gui_conf: GuiSettingsContainer,
        send_tx: Sender<String>,
        gui_event_tx: Sender<GuiEvent>,
        record_options_tx: Sender<RecordOptions>,
    ) -> Self {
        gui_event_tx
            .send(GuiEvent::SetRawTrafficOptions(
                gui_conf.raw_traffic_options.clone(),
            ))
            .expect("Failed to send raw traffic options");
        gui_event_tx
            .send(GuiEvent::SetBufferSize(gui_conf.plot_options.buffer_size))
            .expect("Failed to send buffer size");
        Self {
            connected_to_device: false,
            picked_path: PathBuf::new(),
            device: "".to_string(),
            old_device: "".to_string(),
            data: DataContainer::default(),
            console: vec![Print::Message(
                "waiting for serial connection..,".to_owned(),
            )],
            connected_lock,
            device_lock,
            devices_lock,
            device_idx: 0,
            serial_devices: devices,
            print_lock,
            gui_conf,
            data_lock,
            send_tx,
            gui_event_tx,
            plot_serial_display_ratio: 0.75,
            command: "".to_string(),
            save_raw: false,
            history: vec![],
            index: 0,
            plot_location: None,
            do_not_show_clear_warning: false,
            show_warning_window: WindowFeedback::None,
            need_initialize: false,
            right_panel_expanded: true,
            active_tab: Some(GuiTabs::PlotOptions),
            record_options_tx,
        }
    }

    pub fn clear_warning_window(&mut self, ctx: &egui::Context) -> WindowFeedback {
        let mut window_feedback = WindowFeedback::Waiting;
        egui::Window::new("Attention!")
            .fixed_pos(Pos2 { x: 800.0, y: 450.0 })
            .fixed_size(Vec2 { x: 400.0, y: 200.0 })
            .anchor(Align2::CENTER_CENTER, Vec2 { x: 0.0, y: 0.0 })
            .collapsible(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label("Changing devices will clear all data.");
                    ui.label("How do you want to proceed?");
                    ui.add_space(20.0);
                    ui.checkbox(&mut self.do_not_show_clear_warning, "Remember my decision.");
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        ui.add_space(130.0);
                        if ui.button("Continue & Clear").clicked() {
                            window_feedback = WindowFeedback::Clear;
                        }
                        if ui.button("Cancel").clicked() {
                            window_feedback = WindowFeedback::Cancel;
                        }
                    });
                    ui.add_space(5.0);
                });
            });
        window_feedback
    }

    fn console_text(&self, packet: &crate::data::Packet) -> Option<String> {
        match (
            self.gui_conf.raw_traffic_options.show_sent_cmds,
            self.gui_conf.raw_traffic_options.show_timestamps,
            &packet.direction,
        ) {
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

    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let panel_height = ui.available_size().y;
            let spacing = 5.0;

            ui.add_space(spacing);
            ui.horizontal_centered(|ui| {
                // ui.add_space(border);
                ui.vertical(|ui| {
                    self.plots_ui(ui);

                    let separator_response = ui.separator();
                    let separator = ui
                        .interact(
                            separator_response.rect,
                            separator_response.id,
                            Sense::click_and_drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::ResizeVertical);

                    let resize_y = separator.drag_delta().y;

                    if separator.double_clicked() {
                        self.plot_serial_display_ratio = 0.75;
                    }
                    self.plot_serial_display_ratio =
                        (self.plot_serial_display_ratio + resize_y / panel_height).clamp(0.1, 0.9);

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_value(
                                &mut self.active_tab,
                                Some(GuiTabs::PlotOptions),
                                "Plot Options",
                            )
                            .double_clicked()
                        {
                            self.active_tab = None
                        };

                        if ui
                            .selectable_value(
                                &mut self.active_tab,
                                Some(GuiTabs::RawTraffic),
                                "Raw Traffic",
                            )
                            .double_clicked()
                        {
                            self.active_tab = None
                        };

                        if ui
                            .selectable_value(
                                &mut self.active_tab,
                                Some(GuiTabs::Commands),
                                "Commands",
                            )
                            .double_clicked()
                        {
                            self.active_tab = None
                        };

                        if ui
                            .selectable_value(&mut self.active_tab, Some(GuiTabs::Record), "Record")
                            .double_clicked()
                        {
                            self.active_tab = None
                        };

                        ui.add_space(ui.available_width() - 25.0);

                        if ui
                            .selectable_label(
                                false,
                                egui::RichText::new(if self.active_tab.is_none() {
                                    egui_phosphor::regular::CARET_UP
                                } else {
                                    egui_phosphor::regular::CARET_DOWN
                                }),
                            )
                            .clicked()
                        {
                            self.active_tab = if self.active_tab.is_none() {
                                Some(GuiTabs::PlotOptions)
                            } else {
                                None
                            }
                        }
                    });

                    match self.active_tab {
                        Some(tab) => {
                            ui.separator();
                            ui.add_space(spacing);
                            match tab {
                                GuiTabs::RawTraffic => {
                                    self.serial_raw_traffic_ui(ui);
                                }
                                GuiTabs::Commands => {
                                    self.commands_gui(ui);
                                }
                                GuiTabs::PlotOptions => {
                                    self.plot_options_ui(ui);
                                }
                                GuiTabs::Record => {
                                    self.record_gui(ui);
                                }
                            }
                        }
                        None => (),
                    };
                });
                // ui.add_space(border);
            });
        });
    }

    fn draw_side_panel(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::show_animated_between(
            ctx,
            self.right_panel_expanded,
            SidePanel::right("settings panel collapsed")
                .min_width(0.0)
                .resizable(false),
            SidePanel::right("settings panel expanded")
                .exact_width(RIGHT_PANEL_WIDTH)
                .resizable(false),
            |ui, how_expanded| {
                // ui.set_visible(true);
                if how_expanded == 0.0 {
                    ui.add_space(10.0);
                    if ui
                        .button(egui::RichText::new(
                            egui_phosphor::regular::CARET_LEFT.to_string(),
                        ))
                        .clicked()
                    {
                        self.right_panel_expanded = true;
                    };
                } else {
                    ui.horizontal(|ui| {
                        if ui
                            .heading("Serial Monitor")
                            .interact(egui::Sense::click())
                            .clicked()
                        {
                            self.right_panel_expanded = false;
                        };
                        self.paint_connection_indicator(ui);
                    });
                    ui.add_space(5.0);
                    egui::ScrollArea::vertical()
                        .id_source("settings scroll area")
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            self.serial_settings_ui(ui, ctx);
                            ui.add_space(15.0);
                            self.plot_settings_ui(ui, ctx);
                            ui.add_space(20.0);
                            ui.separator();
                            self.debug_console_ui(ui);
                        });
                }
            },
        );
    }

    fn paint_connection_indicator(&self, ui: &mut egui::Ui) {
        let (color, color_stroke) = if !self.connected_to_device {
            ui.add(egui::Spinner::new());
            (egui::Color32::DARK_RED, egui::Color32::RED)
        } else {
            (egui::Color32::DARK_GREEN, egui::Color32::GREEN)
        };

        let radius = ui.spacing().interact_size.y * 0.375;
        let center = egui::pos2(
            ui.next_widget_position().x + ui.spacing().interact_size.x * 0.5,
            ui.next_widget_position().y,
        );
        ui.painter()
            .circle(center, radius, color, egui::Stroke::new(1.0, color_stroke));
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Ok(read_guard) = self.connected_lock.read() {
            self.connected_to_device = *read_guard;
        }

        self.draw_side_panel(ctx, frame);
        self.draw_central_panel(ctx);
        ctx.request_repaint();

        self.gui_conf.x = ctx.used_size().x;
        self.gui_conf.y = ctx.used_size().y;

        // Check for returned screenshot:
        let screenshot = ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    return Some(image.clone());
                }
            }
            None
        });

        if let (Some(screenshot), Some(plot_location)) = (screenshot, self.plot_location) {
            if let Some(mut path) = rfd::FileDialog::new().save_file() {
                path.set_extension("png");

                // for a full size application, we should put this in a different thread,
                // so that the GUI doesn't lag during saving

                let pixels_per_point = ctx.pixels_per_point();
                let plot = screenshot.region(&plot_location, Some(pixels_per_point));
                // save the plot to png
                image::save_buffer(
                    &path,
                    plot.as_raw(),
                    plot.width() as u32,
                    plot.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                eprintln!("Image saved to {path:?}.");
            }
        }

        std::thread::sleep(Duration::from_millis((1000.0 / MAX_FPS) as u64));
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        save_serial_settings(&self.serial_devices);
        if let Err(err) = self.gui_conf.save(&APP_INFO, PREFS_KEY) {
            println!("gui settings save failed: {:?}", err);
        }
    }
}

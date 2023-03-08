use crate::data::{DataContainer, SerialDirection};
use crate::toggle::toggle;
use crate::Device;
use crate::{vec2, APP_INFO, PREFS_KEY};
use core::f32;
use eframe::egui::panel::Side;
use eframe::egui::plot::{Legend, Line, Plot, PlotPoints};
use eframe::egui::{global_dark_light_mode_buttons, ColorImage, FontFamily, FontId, Vec2, Visuals};
use eframe::glow::HasContext;
use eframe::{egui, glow, Storage};
use image::{ImageResult, RgbaImage};
use preferences::Preferences;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

const MAX_FPS: f64 = 60.0;

const DEFAULT_FONT_ID: FontId = FontId::new(14.0, FontFamily::Monospace);
const RIGHT_PANEL_WIDTH: f32 = 350.0;
const BAUD_RATES: &[u32] = &[
    300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 74880, 115200, 230400, 128000, 460800,
    576000, 921600,
];

#[derive(Clone)]
#[allow(unused)]
pub enum Print {
    Empty,
    Message(String),
    Error(String),
    Debug(String),
    Ok(String),
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
pub struct GuiSettingsContainer {
    pub device: String,
    pub baud: u32,
    pub debug: bool,
    pub x: f32,
    pub y: f32,
    pub dark_mode: bool,
}

impl Default for GuiSettingsContainer {
    fn default() -> Self {
        Self {
            device: "".to_string(),
            baud: 115_200,
            debug: true,
            x: 1600.0,
            y: 900.0,
            dark_mode: true,
        }
    }
}

pub fn load_gui_settings() -> GuiSettingsContainer {
    GuiSettingsContainer::load(&APP_INFO, PREFS_KEY).unwrap_or_else(|_| {
        let gui_settings = GuiSettingsContainer::default();
        // save default settings
        if gui_settings.save(&APP_INFO, PREFS_KEY).is_err() {
            println!("failed to save gui_settings");
        }
        gui_settings
    })
}

#[derive(Default)]
struct PlotSize {
    lower_bound_x: f32,
    lower_bound_y: f32,
    width: f32,
    height: f32,
}

pub struct MyApp {
    ready: bool,
    command: String,
    device: String,
    baud_rate: u32,
    plotting_range: i32,
    console: Vec<Print>,
    picked_path: PathBuf,
    picked_path_plot: PathBuf,
    plot_size: PlotSize,
    data: DataContainer,
    gui_conf: GuiSettingsContainer,
    print_lock: Arc<RwLock<Vec<Print>>>,
    device_lock: Arc<RwLock<Device>>,
    devices_lock: Arc<RwLock<Vec<String>>>,
    connected_lock: Arc<RwLock<bool>>,
    data_lock: Arc<RwLock<DataContainer>>,
    save_tx: Sender<PathBuf>,
    send_tx: Sender<String>,
    clear_tx: Sender<bool>,
    history: Vec<String>,
    index: usize,
    eol: String,
    show_sent_cmds: bool,
    show_timestamps: bool,
    save_raw: bool,
    save_plot: bool,
    plot_to_save: Option<ColorImage>,
}

impl MyApp {
    pub fn new(
        print_lock: Arc<RwLock<Vec<Print>>>,
        data_lock: Arc<RwLock<DataContainer>>,
        device_lock: Arc<RwLock<Device>>,
        devices_lock: Arc<RwLock<Vec<String>>>,
        connected_lock: Arc<RwLock<bool>>,
        gui_conf: GuiSettingsContainer,
        save_tx: Sender<PathBuf>,
        send_tx: Sender<String>,
        clear_tx: Sender<bool>,
    ) -> Self {
        Self {
            ready: false,
            picked_path: PathBuf::new(),
            picked_path_plot: PathBuf::new(),
            device: "".to_string(),
            data: DataContainer::default(),
            console: vec![Print::Message(
                "waiting for serial connection..,".to_owned(),
            )],
            connected_lock,
            device_lock,
            devices_lock,
            print_lock,
            gui_conf,
            data_lock,
            save_tx,
            send_tx,
            clear_tx,
            plotting_range: -1,
            command: "".to_string(),
            baud_rate: 9600,
            show_sent_cmds: true,
            show_timestamps: true,
            save_raw: true,
            eol: "\\r\\n".to_string(),
            history: vec![],
            index: 0,
            save_plot: false,
            plot_to_save: None,
            plot_size: PlotSize::default(),
        }
    }

    fn console_text(&self, packet: &crate::data::Packet) -> Option<String> {
        match (self.show_sent_cmds, self.show_timestamps, &packet.direction) {
            (true, true, _) => Some(format!(
                "[{}] t + {:.3}s: {}",
                packet.direction,
                packet.time as f32 / 1000.0,
                packet.payload
            )),
            (true, false, _) => Some(format!("[{}]: {}", packet.direction, packet.payload)),
            (false, true, SerialDirection::Receive) => Some(format!(
                "t + {:.3}s: {}",
                packet.time as f32 / 1000.0,
                packet.payload
            )),
            (false, false, SerialDirection::Receive) => Some(packet.payload.clone()),
            (_, _, _) => None,
        }
    }

    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let height = ui.available_size().y * 0.45;
            let border = 10.0;
            let spacing = (ui.available_size().y - 2.0 * height) / 3.5 - border;
            let width = ui.available_size().x - 2.0 * border - RIGHT_PANEL_WIDTH;
            // lets set the relative plot size and location for plot saving purposes
            self.plot_size.lower_bound_x = border / ui.available_size().x;
            self.plot_size.lower_bound_y =
                (ui.available_size().y * 0.55 - spacing) / ui.available_size().y;
            self.plot_size.width = width / ui.available_size().x;
            self.plot_size.height = height / ui.available_size().y;

            ui.add_space(spacing);
            ui.horizontal(|ui| {
                ui.add_space(border);
                ui.vertical(|ui| {
                    if let Ok(read_guard) = self.data_lock.read() {
                        self.data = read_guard.clone();
                    }

                    let mut graphs: Vec<Vec<[f64; 2]>> = vec![vec![]; self.data.dataset.len()];
                    let window: usize = if self.plotting_range == -1
                        || self.data.dataset[0].len() <= self.plotting_range as usize
                    {
                        0
                    } else {
                        self.data.dataset[0].len() - self.plotting_range as usize
                    };

                    for i in window..self.data.dataset[0].len() {
                        for (graph, data) in graphs.iter_mut().zip(&self.data.dataset) {
                            //graph.push([i as f64, data[i] as f64]);
                            if self.data.time.len() == data.len() {
                                graph.push([self.data.time[i] as f64, data[i] as f64]);
                            } else {
                                // not same length
                                // println!("not same length in gui! length self.data.time = {}, length data = {}", self.data.time.len(), data.len())
                            }
                        }
                    }

                    let t_fmt = |x, _range: &RangeInclusive<f64>| format!("{:4.2} s", x);
                    let s_fmt = move |y, _range: &RangeInclusive<f64>| format!("{:4.2} [a.u.]", y);
                    let signal_plot = Plot::new("data")
                        .height(height)
                        .width(width)
                        .legend(Legend::default())
                        .y_axis_formatter(s_fmt)
                        .x_axis_formatter(t_fmt)
                        .min_size(vec2(50.0, 100.0));

                    signal_plot.show(ui, |signal_plot_ui| {
                        for (i, graph) in graphs.iter().enumerate() {
                            signal_plot_ui.line(
                                Line::new(PlotPoints::from(graph.clone()))
                                    .name(format!("Column {}", i)),
                            );
                        }
                    });

                    let num_rows = self.data.raw_traffic.len();
                    let row_height = ui.text_style_height(&egui::TextStyle::Body);

                    ui.add_space(spacing);
                    ui.separator();
                    ui.add_space(spacing);

                    egui::ScrollArea::vertical()
                        .id_source("serial_output")
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .always_show_scroll(true)
                        .enable_scrolling(true)
                        .max_height(height - spacing)
                        .min_scrolled_height(height - spacing)
                        .max_width(width)
                        .show_rows(ui, row_height, num_rows, |ui, _row_range| {
                            let content: String = self
                                .data
                                .raw_traffic
                                .iter()
                                .map(|packet| match self.console_text(packet) {
                                    None => "".to_string(),
                                    Some(text) => text + "\n",
                                })
                                .collect();
                            let color = if self.gui_conf.dark_mode {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::BLACK
                            };
                            ui.add(
                                egui::TextEdit::multiline(&mut content.as_str())
                                    .font(DEFAULT_FONT_ID) // for cursor height
                                    .lock_focus(true)
                                    .text_color(color)
                                    .desired_width(width),
                            );
                        });
                    ui.add_space(spacing / 2.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.command)
                                .desired_width(width - 50.0)
                                .code_editor(),
                        );
                        if ui.input(|i| i.key_pressed(egui::Key::Enter))
                            || ui.button("Send").clicked()
                        {
                            // send command
                            self.history.push(self.command.clone());
                            self.index = self.history.len() - 1;
                            if let Err(err) = self.send_tx.send(self.command.clone() + &self.eol) {
                                print_to_console(
                                    &self.print_lock,
                                    Print::Error(format!("send_tx thread send failed: {:?}", err)),
                                );
                            }
                        }
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.index = self.index.saturating_sub(1);
                        if !self.history.is_empty() {
                            self.command = self.history[self.index].clone();
                        }
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.index = std::cmp::min(self.index + 1, self.history.len() - 1);
                        if !self.history.is_empty() {
                            self.command = self.history[self.index].clone();
                        }
                    }

                    ctx.request_repaint()
                });
                ui.add_space(border);
            });
        });
    }

    fn draw_side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::new(Side::Right, "settings panel")
            .min_width(RIGHT_PANEL_WIDTH)
            .max_width(RIGHT_PANEL_WIDTH)
            .resizable(false)
            //.default_width(right_panel_width)
            .show(ctx, |ui| {
                ui.add_enabled_ui(true, |ui| {
                    ui.set_visible(true);
                    ui.horizontal(|ui| {
                        ui.heading("Serial Monitor");
                        let color_stroke;
                        let color;
                        if !self.ready {
                            ui.add(egui::Spinner::new());
                            color = egui::Color32::DARK_RED;
                            color_stroke = egui::Color32::RED;
                        } else {
                            color = egui::Color32::DARK_GREEN;
                            color_stroke = egui::Color32::GREEN;
                        }
                        let radius = &ui.spacing().interact_size.y * 0.375;
                        let center = egui::pos2(
                            ui.next_widget_position().x + &ui.spacing().interact_size.x * 0.5,
                            ui.next_widget_position().y,
                        );
                        ui.painter().circle(
                            center,
                            radius,
                            color,
                            egui::Stroke::new(1.0, color_stroke),
                        );
                    });

                    let devices: Vec<String> = if let Ok(read_guard) = self.devices_lock.read() {
                        read_guard.clone()
                    } else {
                        vec![]
                    };

                    if !devices.contains(&self.device) {
                        self.device.clear();
                    }

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let dev_text = self.device.replace("/dev/tty.", "");
                        egui::ComboBox::from_id_source("Device")
                            .selected_text(dev_text)
                            .width(RIGHT_PANEL_WIDTH * 0.92 - 155.0)
                            .show_ui(ui, |ui| {
                                devices
                                    .into_iter()
                                    // on macOS each device appears as /dev/tty.* and /dev/cu.*
                                    // we only display the /dev/tty.* here
                                    .filter(|dev| !dev.contains("/dev/cu."))
                                    .for_each(|dev| {
                                        // this makes the names shorter in the UI on UNIX and UNIX-like platforms
                                        let dev_text = dev.replace("/dev/tty.", "");
                                        ui.selectable_value(&mut self.device, dev, dev_text);
                                    });
                            });
                        egui::ComboBox::from_id_source("Baud Rate")
                            .selected_text(format!("{}", self.baud_rate))
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                BAUD_RATES.iter().for_each(|baud_rate| {
                                    ui.selectable_value(
                                        &mut self.baud_rate,
                                        *baud_rate,
                                        baud_rate.to_string(),
                                    );
                                });
                            });
                        let connect_text = if self.ready { "Disconnect" } else { "Connect" };
                        if ui.button(connect_text).clicked() {
                            if let Ok(mut write_guard) = self.device_lock.write() {
                                if self.ready {
                                    write_guard.name.clear();
                                } else {
                                    write_guard.name = self.device.clone();
                                    write_guard.baud_rate = self.baud_rate;
                                }
                            }
                        }
                    });

                    ui.add_space(20.0);

                    egui::Grid::new("upper")
                        .num_columns(2)
                        .spacing(Vec2 { x: 10.0, y: 10.0 })
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Plotting range [#]: ");
                            if ui
                                .add(egui::DragValue::new(&mut self.plotting_range))
                                .lost_focus()
                            {
                                //gui_states.push(GuiState::TBegin(self.tera_flash_conf.t_begin));
                            };
                            ui.end_row();
                            if ui.button("Save Data").clicked() {
                                if let Some(path) = rfd::FileDialog::new().save_file() {
                                    self.picked_path = path;
                                    self.picked_path.set_extension("csv");
                                    if let Err(e) = self.save_tx.send(self.picked_path.clone()) {
                                        print_to_console(
                                            &self.print_lock,
                                            Print::Error(format!(
                                                "save_tx thread send failed: {:?}",
                                                e
                                            )),
                                        );
                                    }
                                }
                            };
                            if ui
                                .button("Save Plot")
                                .on_hover_text("Save an image of the plot - this is experimental!")
                                .clicked()
                            {
                                if let Some(mut path) = rfd::FileDialog::new().save_file() {
                                    path.set_extension("png");
                                    self.save_plot = true;
                                    self.picked_path_plot = path;
                                }
                            }
                            ui.end_row();
                            if ui.button("Clear Data").clicked() {
                                print_to_console(
                                    &self.print_lock,
                                    Print::Ok("Cleared recorded data".to_string()),
                                );
                                match self.clear_tx.send(true) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        print_to_console(
                                            &self.print_lock,
                                            Print::Error(format!(
                                                "clear_tx thread send failed: {:?}",
                                                err
                                            )),
                                        );
                                    }
                                }
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
                            ui.add(
                                egui::TextEdit::singleline(&mut self.eol)
                                    .desired_width(ui.available_width() * 0.9),
                            );
                            // ui.checkbox(&mut self.gui_conf.debug, "Debug Mode");
                            ui.end_row();
                            global_dark_light_mode_buttons(ui);
                            self.gui_conf.dark_mode = ui.visuals() == &Visuals::dark();
                            ui.end_row();
                            ui.label("");
                            ui.end_row();
                        });
                });
                if let Ok(read_guard) = self.print_lock.read() {
                    self.console = read_guard.clone();
                }
                let num_rows = self.console.len();
                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                egui::ScrollArea::vertical()
                    .id_source("console_scroll_area")
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .max_height(row_height * 15.5)
                    .show_rows(ui, row_height, num_rows, |ui, _row_range| {
                        let content: String = self
                            .console
                            .iter()
                            .map(|row| match row.scroll_area_message(&self.gui_conf) {
                                None => "".to_string(),
                                Some(msg) => msg.label + msg.content.as_str() + "\n",
                            })
                            .collect();
                        ui.add(
                            egui::TextEdit::multiline(&mut content.as_str())
                                .font(DEFAULT_FONT_ID) // for cursor height
                                .lock_focus(true), // TODO: add a layouter to highlight the labels
                        );
                    });
            });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(read_guard) = self.connected_lock.read() {
            self.ready = *read_guard;
        }

        self.draw_central_panel(ctx);
        self.draw_side_panel(ctx);

        self.gui_conf.x = ctx.used_size().x;
        self.gui_conf.y = ctx.used_size().y;

        if let Some(plot_to_save) = self.plot_to_save.take() {
            // maybe we should put this in a different thread, so that the GUI
            // doesn't lag during saving
            match save_image(&plot_to_save, &self.picked_path_plot) {
                Ok(_) => {
                    print_to_console(
                        &self.print_lock,
                        Print::Ok(format!("saved plot to {:?} ", self.picked_path_plot)),
                    );
                }
                Err(e) => {
                    print_to_console(
                        &self.print_lock,
                        Print::Error(format!(
                            "failed to plot to {:?}: {:?}",
                            self.picked_path_plot, e
                        )),
                    );
                }
            }
        }

        std::thread::sleep(Duration::from_millis((1000.0 / MAX_FPS) as u64));
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        if let Err(err) = self.gui_conf.save(&APP_INFO, PREFS_KEY) {
            println!("gui settings save failed: {:?}", err);
        }
    }

    #[allow(unsafe_code)]
    fn post_rendering(&mut self, screen_size_px: [u32; 2], frame: &eframe::Frame) {
        // this is inspired by the Egui screenshot example

        if !self.save_plot {
            return;
        }

        self.save_plot = false;
        if let Some(gl) = frame.gl() {
            let [window_width, window_height] = screen_size_px;

            // we needed the relative values here, because we need to have them in relation to the
            // screen_size_px.
            // calculating with absolut px values does not always work (for example with retina
            // display MacBooks we have different absolute values than with external displays)
            // using relative values, we have a working solution for all cases
            let w_lower = self.plot_size.lower_bound_x * window_width as f32;
            let h_lower = self.plot_size.lower_bound_y * window_height as f32;
            let w = self.plot_size.width * window_width as f32;
            let h = self.plot_size.height * window_height as f32;

            let mut buf = vec![0u8; w as usize * h as usize * 4];
            let pixels = glow::PixelPackData::Slice(&mut buf[..]);
            unsafe {
                gl.read_pixels(
                    w_lower as i32,
                    h_lower as i32,
                    w as i32,
                    h as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    pixels,
                );
            }

            // Flip vertically:
            let buf: Vec<u8> = buf
                .chunks(w as usize * 4)
                .rev()
                .flat_map(|chunk| chunk.iter())
                .copied()
                .collect();
            self.plot_to_save = Some(ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                &buf[..],
            ));
        }
    }
}

fn save_image(img: &ColorImage, file_path: &PathBuf) -> ImageResult<()> {
    let height = img.height();
    let width = img.width();
    let raw: Vec<u8> = img
        .pixels
        .iter()
        .flat_map(|p| vec![p.r(), p.g(), p.b(), p.a()])
        .collect();
    let img_to_save = RgbaImage::from_raw(width as u32, height as u32, raw)
        .expect("container should have the right size for the image dimensions");
    img_to_save.save(file_path)
}

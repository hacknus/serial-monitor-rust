use super::*;
use eframe::egui::{self, Button, TextEdit, TextStyle};
use rfd::MessageDialog;

impl MyApp {
    pub fn serial_settings_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.need_initialize = false;

        let devices: Vec<String> = if let Ok(read_guard) = self.devices_lock.read() {
            read_guard.clone()
        } else {
            vec![]
        };

        if !devices.contains(&self.device) {
            self.device.clear();
        }

        ui.heading("串口设置");
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label("Device");
            ui.add_space(130.0);
            ui.label("Baud");
        });

        let old_name = self.device.clone();
        ui.horizontal(|ui| {
            let dev_text = self.device.replace("/dev/tty.", "");
            ui.horizontal(|ui| {
                ui.set_enabled(!self.connected_to_device);
                let _response = egui::ComboBox::from_id_source("Device")
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
                    })
                    .response;
                // let selected_new_device = response.changed();  //somehow this does not work
                // if selected_new_device {
                if old_name != self.device {
                    if !self.data.time.is_empty() {
                        self.show_warning_window = WindowFeedback::Waiting;
                        self.old_device = old_name;
                    } else {
                        self.show_warning_window = WindowFeedback::Clear;
                    }
                }
            });
            match self.show_warning_window {
                WindowFeedback::None => {}
                WindowFeedback::Waiting => {
                    self.show_warning_window = self.clear_warning_window(ctx);
                }
                WindowFeedback::Clear => {
                    // new device selected, check in previously used devices
                    let mut device_is_already_saved = false;
                    for (idx, dev) in self.serial_devices.devices.iter().enumerate() {
                        if dev.name == self.device {
                            // this is the device!
                            self.device = dev.name.clone();
                            self.device_idx = idx;
                            self.need_initialize = true;
                            device_is_already_saved = true;
                        }
                    }
                    if !device_is_already_saved {
                        // create new device in the archive
                        let mut device = Device::default();
                        device.name = self.device.clone();
                        self.serial_devices.devices.push(device);
                        // self.serial_devices.number_of_plots.push(1);
                        // self.serial_devices
                        //     .labels
                        //     .push(vec!["Column 0".to_string()]);
                        self.device_idx = self.serial_devices.devices.len() - 1;
                        save_serial_settings(&self.serial_devices);
                    }
                    self.gui_event_tx
                        .send(GuiEvent::Clear)
                        .expect("failed to send clear after choosing new device");
                    // need to clear the data here such that we don't get errors in the gui (plot)
                    self.data = DataContainer::default();
                    self.show_warning_window = WindowFeedback::None;
                }
                WindowFeedback::Cancel => {
                    self.device = self.old_device.clone();
                    self.show_warning_window = WindowFeedback::None;
                }
            }
            egui::ComboBox::from_id_source("Baud Rate")
                .selected_text(format!(
                    "{}",
                    self.serial_devices.devices[self.device_idx].baud_rate
                ))
                .width(80.0)
                .show_ui(ui, |ui| {
                    BAUD_RATES.iter().for_each(|baud_rate| {
                        ui.selectable_value(
                            &mut self.serial_devices.devices[self.device_idx].baud_rate,
                            *baud_rate,
                            baud_rate.to_string(),
                        );
                    });
                });
            let connect_text = if self.connected_to_device {
                "Disconnect"
            } else {
                "Connect"
            };
            if ui.button(connect_text).clicked() {
                if let Ok(mut device) = self.device_lock.write() {
                    if self.connected_to_device {
                        device.name.clear();
                    } else {
                        device.name = self.serial_devices.devices[self.device_idx].name.clone();
                        device.baud_rate = self.serial_devices.devices[self.device_idx].baud_rate;
                    }
                }
            }
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label("Data Bits");
            ui.add_space(5.0);
            ui.label("Parity");
            ui.add_space(20.0);
            ui.label("Stop Bits");
            ui.label("Flow Control");
            ui.label("Timeout");
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("Data Bits")
                .selected_text(
                    self.serial_devices.devices[self.device_idx]
                        .data_bits
                        .to_string(),
                )
                .width(30.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].data_bits,
                        DataBits::Eight,
                        DataBits::Eight.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].data_bits,
                        DataBits::Seven,
                        DataBits::Seven.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].data_bits,
                        DataBits::Six,
                        DataBits::Six.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].data_bits,
                        DataBits::Five,
                        DataBits::Five.to_string(),
                    );
                });
            egui::ComboBox::from_id_source("Parity")
                .selected_text(
                    self.serial_devices.devices[self.device_idx]
                        .parity
                        .to_string(),
                )
                .width(30.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].parity,
                        Parity::None,
                        Parity::None.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].parity,
                        Parity::Odd,
                        Parity::Odd.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].parity,
                        Parity::Even,
                        Parity::Even.to_string(),
                    );
                });
            egui::ComboBox::from_id_source("Stop Bits")
                .selected_text(
                    self.serial_devices.devices[self.device_idx]
                        .stop_bits
                        .to_string(),
                )
                .width(30.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].stop_bits,
                        StopBits::One,
                        StopBits::One.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].stop_bits,
                        StopBits::Two,
                        StopBits::Two.to_string(),
                    );
                });
            egui::ComboBox::from_id_source("Flow Control")
                .selected_text(
                    self.serial_devices.devices[self.device_idx]
                        .flow_control
                        .to_string(),
                )
                .width(75.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].flow_control,
                        FlowControl::None,
                        FlowControl::None.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].flow_control,
                        FlowControl::Hardware,
                        FlowControl::Hardware.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].flow_control,
                        FlowControl::Software,
                        FlowControl::Software.to_string(),
                    );
                });
            egui::ComboBox::from_id_source("Timeout")
                .selected_text(
                    self.serial_devices.devices[self.device_idx]
                        .timeout
                        .as_millis()
                        .to_string(),
                )
                .width(55.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].timeout,
                        Duration::from_millis(0),
                        "0",
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].timeout,
                        Duration::from_millis(10),
                        "10",
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].timeout,
                        Duration::from_millis(100),
                        "100",
                    );
                    ui.selectable_value(
                        &mut self.serial_devices.devices[self.device_idx].timeout,
                        Duration::from_millis(1000),
                        "1000",
                    );
                });
        });
    }

    pub fn plot_settings_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("画图设置");
        ui.add_space(5.0);
        egui::Grid::new("upper")
            .num_columns(2)
            .spacing(Vec2 { x: 10.0, y: 10.0 })
            .striped(true)
            .show(ui, |ui| {
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Save CSV",
                        egui_phosphor::regular::FLOPPY_DISK
                    )))
                    .on_hover_text("Save Plot Data to CSV.")
                    .clicked()
                    || ui.input_mut(|i| i.consume_shortcut(&SAVE_FILE_SHORTCUT))
                {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        self.picked_path = path;
                        self.picked_path.set_extension("csv");
                        if let Err(e) = self.gui_event_tx.send(GuiEvent::SaveCSV(FileOptions {
                            file_path: self.picked_path.clone(),
                            save_absolute_time: self.gui_conf.save_absolute_time,
                            save_raw_traffic: self.save_raw,
                        })) {
                            print_to_console(
                                &self.print_lock,
                                Print::Error(format!("save_tx thread send failed: {:?}", e)),
                            );
                        }
                    }
                };

                if ui
                    .button(egui::RichText::new(format!(
                        "{} Save Plot",
                        egui_phosphor::regular::FLOPPY_DISK
                    )))
                    .on_hover_text("Save an image of the Plot.")
                    .clicked()
                    || ui.input_mut(|i| i.consume_shortcut(&SAVE_PLOT_SHORTCUT))
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                }
                ui.end_row();
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Clear Data",
                        egui_phosphor::regular::X
                    )))
                    .on_hover_text("Clear Data from Plot.")
                    .clicked()
                    || ui.input_mut(|i| i.consume_shortcut(&CLEAR_PLOT_SHORTCUT))
                {
                    print_to_console(
                        &self.print_lock,
                        Print::Ok("Cleared recorded Data".to_string()),
                    );
                    if let Err(err) = self.gui_event_tx.send(GuiEvent::Clear) {
                        print_to_console(
                            &self.print_lock,
                            Print::Error(format!("clear_tx thread send failed: {:?}", err)),
                        );
                    }
                    // need to clear the data here in order to prevent errors in the gui (plot)
                    self.data = DataContainer::default();
                    self.gui_event_tx
                        .send(GuiEvent::SetNames(
                            self.gui_conf.plot_options.labels.clone(),
                        ))
                        .expect("Failed to send names");
                }
                ui.end_row();
                ui.label("Save Raw Traffic");
                ui.add(toggle(&mut self.save_raw))
                    .on_hover_text("Save second CSV containing raw traffic.")
                    .changed();
                ui.end_row();
                ui.label("Save Absolute Time");
                ui.add(toggle(&mut self.gui_conf.save_absolute_time))
                    .on_hover_text("Save absolute time in CSV.");
                ui.end_row();
            });
        ui.add_space(25.0);
        global_dark_light_mode_buttons(ui);
        ui.add_space(25.0);
        self.gui_conf.dark_mode = ui.visuals() == &Visuals::dark();
        ui.horizontal(|ui| {
            if ui.button("Clear Device History").clicked() {
                self.serial_devices = SerialDevices::default();
                self.device.clear();
                self.device_idx = 0;
                clear_serial_settings();
            }
        });
    }

    pub fn debug_console_ui(&mut self, ui: &mut egui::Ui) {
        if let Ok(read_guard) = self.print_lock.read() {
            self.console = read_guard.clone();
        }
        let num_rows = self.console.len();
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        ui.label("Debug Info:");
        ui.add_space(10.0);
        egui::ScrollArea::vertical()
            .id_source("console_scroll_area")
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .max_height(row_height * 15.5)
            .show_rows(ui, row_height, num_rows, |ui, _row_range| {
                let content: String = self
                    .console
                    .iter()
                    .flat_map(|row| row.scroll_area_message(&self.gui_conf))
                    .map(|msg| msg.label + msg.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                // we need to add it as one multiline object, such that we can select and copy
                // text over multiple lines
                ui.add(
                    egui::TextEdit::multiline(&mut content.as_str())
                        .font(DEFAULT_FONT_ID) // for cursor height
                        .lock_focus(true), // TODO: add a layouter to highlight the labels
                );
            });
    }

    pub fn plots_ui(&mut self, ui: &mut egui::Ui) -> egui::InnerResponse<()> {
        let border = 10.0;
        let width = ui.available_size().x - 2.0 * border;
        let height = if self.active_tab.is_none() {
            ui.available_height() - 18.0
        } else {
            ui.available_height() * self.plot_serial_display_ratio
        };
        let plots_height = height;
        // need to subtract 12.0, this seems to be the height of the separator of two adjacent plots
        let plot_height = plots_height / (self.gui_conf.plot_options.number_of_plots as f32) - 12.0;

        if let Ok(read_guard) = self.data_lock.read() {
            self.data = read_guard.clone();
        }

        let mut graphs: Vec<Vec<PlotPoint>> = vec![vec![]; self.data.dataset.len()];
        let window = self.data.dataset[0]
            .len()
            .saturating_sub(self.gui_conf.plot_options.plotting_range);

        for (i, time) in self.data.time[window..].iter().enumerate() {
            let x = if self.gui_conf.plot_options.time_x_axis {
                *time as f64 / 1000.0
            } else {
                (i + 1) as f64
            };
            for (graph, data) in graphs.iter_mut().zip(&self.data.dataset) {
                if self.data.time.len() == data.len() {
                    if let Some(y) = data.get(i + window) {
                        graph.push(PlotPoint { x, y: *y as f64 });
                    }
                }
            }
        }

        // let t_fmt = |x, _n, _range: &RangeInclusive<f64>| format!("{:4.2} s", x);

        ui.vertical_centered_justified(|ui| {
            for graph_idx in 0..self.gui_conf.plot_options.number_of_plots {
                if graph_idx != 0 {
                    ui.separator();
                }

                let signal_plot = Plot::new(format!("data-{graph_idx}"))
                    .height(plot_height)
                    .width(width)
                    .auto_bounds_x()
                    .auto_bounds_y()
                    .legend(Legend::default())
                    .x_grid_spacer(log_grid_spacer(10))
                    .y_grid_spacer(log_grid_spacer(10));

                // .x_axis_formatter(t_fmt);

                let plot_inner = signal_plot.show(ui, |signal_plot_ui| {
                    for (i, graph) in graphs.iter().enumerate() {
                        // this check needs to be here for when we change devices (not very elegant)
                        if i < self.gui_conf.plot_options.labels.len() {
                            signal_plot_ui.line(
                                Line::new(PlotPoints::Owned(graph.to_vec()))
                                    .name(&self.gui_conf.plot_options.labels[i]),
                            );
                        }
                    }
                });

                self.plot_location = Some(plot_inner.response.rect);
            }
        })
    }

    pub fn serial_raw_traffic_ui(&mut self, ui: &mut egui::Ui) {
        let border = 10.0;

        let spacing = 5.0;
        let serial_height = ui.available_size().y - border * 2.0;

        let num_rows = self.data.raw_traffic.len();
        let row_height = ui.text_style_height(&egui::TextStyle::Body);

        let color = if self.gui_conf.dark_mode {
            egui::Color32::WHITE
        } else {
            egui::Color32::BLACK
        };
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.heading("Options");
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .id_source("raw traffic options scroll")
                    .auto_shrink([false; 2])
                    .max_width(150.0)
                    .show(ui, |ui| {
                        if ui
                            .selectable_label(self.gui_conf.raw_traffic_options.enable, "Enable")
                            .clicked()
                        {
                            self.gui_conf.raw_traffic_options.enable =
                                !self.gui_conf.raw_traffic_options.enable;
                            self.gui_event_tx
                                .send(GuiEvent::SetRawTrafficOptions(
                                    self.gui_conf.raw_traffic_options.clone(),
                                ))
                                .expect("Failed to update raw traffic options")
                        };

                        if ui
                            .selectable_label(
                                self.gui_conf.raw_traffic_options.show_sent_cmds,
                                "Show Sent Commands",
                            )
                            .on_hover_text("Show sent commands in console.")
                            .clicked()
                        {
                            self.gui_conf.raw_traffic_options.show_sent_cmds =
                                !self.gui_conf.raw_traffic_options.show_sent_cmds
                        };

                        if ui
                            .selectable_label(
                                self.gui_conf.raw_traffic_options.show_timestamps,
                                "Show Timestamp",
                            )
                            .on_hover_text("Show timestamp in console.")
                            .clicked()
                        {
                            self.gui_conf.raw_traffic_options.show_timestamps =
                                !self.gui_conf.raw_traffic_options.show_timestamps
                        };
                        ui.add_space(10.0);
                        ui.label("EOL character:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.gui_conf.raw_traffic_options.eol)
                                .desired_width(80.0),
                        )
                        .on_hover_text("Configure your EOL character for sent commands..");

                        ui.add_space(10.0);

                        ui.label("Max Recorded Len:");
                        if ui
                            .add(egui::DragValue::new(
                                &mut self.gui_conf.raw_traffic_options.max_len,
                            ))
                            .on_hover_text("Select the number of raw traffic to be recorded.")
                            .changed()
                        {
                            self.gui_event_tx
                                .send(GuiEvent::SetRawTrafficOptions(
                                    self.gui_conf.raw_traffic_options.clone(),
                                ))
                                .expect("Failed to update raw traffic options")
                        }
                    });
            });

            ui.separator();

            let width = ui.available_size().x - 2.0 * border;
            ui.vertical_centered_justified(|ui| {
                egui::ScrollArea::vertical()
                    .id_source("serial_output")
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .enable_scrolling(true)
                    .max_height(serial_height - 2.0 * spacing)
                    .min_scrolled_height(serial_height - spacing)
                    .max_width(width)
                    .show_rows(ui, row_height, num_rows, |ui, row_range| {
                        let content: String = row_range
                            .into_iter()
                            .flat_map(|i| {
                                if self.data.raw_traffic.is_empty() {
                                    None
                                } else {
                                    self.console_text(&self.data.raw_traffic[i])
                                }
                            })
                            .collect();
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
                    let cmd_line = ui.add(
                        egui::TextEdit::singleline(&mut self.command)
                            .desired_width(width - 50.0)
                            .lock_focus(true)
                            .code_editor(),
                    );
                    let cmd_has_lost_focus = cmd_line.lost_focus();
                    let key_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (key_pressed && cmd_has_lost_focus) || ui.button("Send").clicked() {
                        // send command
                        let command = self
                            .command
                            .clone()
                            .replace("\\r", "\r")
                            .replace("\\n", "\n");
                        self.history.push(command.clone());
                        self.index = self.history.len() - 1;
                        let eol = self
                            .gui_conf
                            .raw_traffic_options
                            .eol
                            .replace("\\r", "\r")
                            .replace("\\n", "\n");
                        if let Err(err) = self.send_tx.send(command.clone() + &eol) {
                            print_to_console(
                                &self.print_lock,
                                Print::Error(format!("send_tx thread send failed: {:?}", err)),
                            );
                        }
                        // stay in focus!
                        cmd_line.request_focus();
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
            });
        });
    }

    pub fn plot_options_ui(&mut self, ui: &mut egui::Ui) {
        let spacing = 10.0;
        let linespread = 5.0;
        ui.horizontal_centered(|ui| {
            ui.vertical(|ui| {
                ui.heading("Plot Options");
                ui.add_space(linespread);
                egui::ScrollArea::vertical()
                    .max_width(200.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Buffer size [#]: ");
                            ui.add_space(spacing);

                            if ui
                                .add(
                                    egui::DragValue::new(
                                        &mut self.gui_conf.plot_options.buffer_size,
                                    )
                                    .update_while_editing(false),
                                )
                                .on_hover_text("Set the max recorded buffer size.")
                                .changed()
                            {
                                self.gui_event_tx
                                    .send(GuiEvent::SetBufferSize(
                                        self.gui_conf.plot_options.buffer_size,
                                    ))
                                    .expect("Failed to send buffer size");
                            };
                        });

                        ui.add_space(linespread);

                        ui.horizontal(|ui| {
                            ui.label("Plotting range [#]: ");
                            ui.add_space(spacing);

                            let window_fmt = |val: f64, _range: RangeInclusive<usize>| {
                                if val != usize::MAX as f64 {
                                    val.to_string()
                                } else {
                                    "∞".to_string()
                                }
                            };

                            ui.add(
                        egui::DragValue::new(&mut self.gui_conf.plot_options.plotting_range)
                            .custom_formatter(window_fmt),
                    )
                    .on_hover_text(
                        "Select a window of the last datapoints to be displayed in the plot.",
                    );
                            if ui
                                .button("Full Dataset")
                                .on_hover_text("Show the full dataset.")
                                .clicked()
                            {
                                self.gui_conf.plot_options.plotting_range = usize::MAX;
                            }
                        });

                        ui.add_space(linespread);

                        ui.horizontal(|ui| {
                            ui.label("Number of plots [#]: ");
                            ui.add_space(spacing);

                            if ui
                                .button(egui::RichText::new(
                                    egui_phosphor::regular::ARROW_FAT_LEFT.to_string(),
                                ))
                                .clicked()
                            {
                                self.gui_conf.plot_options.number_of_plots =
                                    (self.gui_conf.plot_options.number_of_plots - 1).clamp(1, 10);
                            }
                            ui.add(
                                egui::DragValue::new(
                                    &mut self.gui_conf.plot_options.number_of_plots,
                                )
                                .clamp_range(1..=10),
                            )
                            .on_hover_text("Select the number of plots to be shown.");
                            if ui
                                .button(egui::RichText::new(
                                    egui_phosphor::regular::ARROW_FAT_RIGHT.to_string(),
                                ))
                                .clicked()
                            {
                                self.gui_conf.plot_options.number_of_plots =
                                    (self.gui_conf.plot_options.number_of_plots + 1).clamp(1, 10);
                            }
                        });

                        ui.add_space(linespread);

                        if ui
                            .selectable_label(
                                self.gui_conf.plot_options.time_x_axis,
                                "Time as X Axis",
                            )
                            .clicked()
                        {
                            self.gui_conf.plot_options.time_x_axis =
                                !self.gui_conf.plot_options.time_x_axis;
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                if ui.button("Reset Labels").clicked() {
                    self.gui_conf.plot_options.labels = self.data.names.clone();
                }
                ui.add_space(linespread);
                if self.data.names.len() == 1 {
                    ui.label("Detected 1 Dataset:");
                } else {
                    ui.label(format!("Detected {} Datasets:", self.data.names.len()));
                }
                ui.add_space(5.0);
                for i in 0..self.data.names.len().min(10) {
                    // if init, set names to what has been stored in the device last time
                    if self.need_initialize {
                        self.gui_event_tx
                            .send(GuiEvent::SetNames(
                                self.gui_conf.plot_options.labels.clone(),
                            ))
                            .expect("Failed to send names");
                        self.need_initialize = false;
                    }
                    if self.gui_conf.plot_options.labels.len() <= i {
                        self.gui_conf
                            .plot_options
                            .labels
                            .push(self.data.names[i].clone());
                        // break;
                    }

                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.gui_conf.plot_options.labels[i])
                                .desired_width(150.0),
                        )
                        .on_hover_text("Use custom names for your Datasets.")
                        .changed()
                    {
                        self.gui_event_tx
                            .send(GuiEvent::SetNames(
                                self.gui_conf.plot_options.labels.clone(),
                            ))
                            .expect("Failed to send names");
                    };
                }
                if self.data.names.len() > 10 {
                    ui.label("Only renaming up to 10 Datasets is currently supported.");
                }
            })
        });
    }

    pub fn record_gui(&mut self, ui: &mut egui::Ui) {
        const LINESPREAD: f32 = 10.0;
        const SPACE: f32 = 15.0;
        let mut input_path = self
            .gui_conf
            .record_options
            .record_path
            .to_str()
            .unwrap_or("")
            .to_owned();

        ui.horizontal(|ui| {
            ui.heading("Record Options");
            ui.add_space(2.0 * SPACE);

            ui.style_mut().text_styles = [(
                TextStyle::Button,
                FontId::new(16.0, FontFamily::Proportional),
            )]
            .into();

            if ui
                .selectable_label(self.gui_conf.record_options.enable, "Start Record")
                .clicked()
            {
                let mut ready = true;
                self.gui_conf
                    .record_options
                    .record_path
                    .set_extension("csv");
                if !self.gui_conf.record_options.enable
                    && self.gui_conf.record_options.record_path.exists()
                {
                    ready = false;
                    let result = MessageDialog::new()
                        .set_title("Overwrite confirm")
                        .set_description(format!(
                            "File {} exists. Do you want to overwrite it?",
                            self.gui_conf
                                .record_options
                                .record_path
                                .to_str()
                                .unwrap_or_default()
                        ))
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .show();
                    match result {
                        rfd::MessageDialogResult::Yes => ready = true,
                        rfd::MessageDialogResult::No => ready = false,
                        _ => (),
                    }
                }
                if ready {
                    self.gui_conf.record_options.enable = !self.gui_conf.record_options.enable;
                    self.record_options_tx
                        .send(self.gui_conf.record_options.clone())
                        .expect("Failed to send record options");
                }
            }

            ui.reset_style();
        });
        ui.add_space(LINESPREAD);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::TextEdit::singleline(&mut input_path)
                        .desired_width(ui.available_width() * 0.8)
                        .hint_text("Enter file name or browse"),
                )
                .changed()
            {
                self.gui_conf.record_options.record_path = PathBuf::from(input_path);
            };

            if ui.button("Browse").clicked() {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    self.gui_conf.record_options.record_path = path;
                    self.gui_conf
                        .record_options
                        .record_path
                        .set_extension("csv");
                }
            }
        });
    }

    pub fn commands_gui(&mut self, ui: &mut egui::Ui) {
        const LINESPREAD: f32 = 10.0;

        ui.heading("Commands");
        ui.add_space(LINESPREAD);
        egui::ScrollArea::vertical()
            .id_source("commands_scroll_area")
            .auto_shrink([false; 2])
            .max_width(ui.available_width())
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        for cmd in &mut self.gui_conf.commands {
                            let name_resp = ui.add(
                                TextEdit::singleline(&mut cmd.name)
                                    .frame(cmd.editing)
                                    .desired_width(0.0)
                                    .clip_text(false),
                            );
                            if name_resp.clicked() || name_resp.has_focus() {
                                cmd.editing = true
                            } else {
                                cmd.editing = false
                            }
                        }
                    });
                    ui.add_space(5.0);
                    ui.vertical(|ui| {
                        self.gui_conf.commands.retain_mut(|cmd| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    TextEdit::singleline(&mut cmd.cmd)
                                        .code_editor()
                                        .lock_focus(false)
                                        .clip_text(true)
                                        .desired_width(ui.available_width() - 90.0),
                                );
                                let send_cmd =
                                    cmd.cmd.clone().replace("\\r", "\r").replace("\\n", "\n");
                                if ui.button("Send").clicked() {
                                    if let Err(err) = self.send_tx.send(send_cmd) {
                                        print_to_console(
                                            &self.print_lock,
                                            Print::Error(format!(
                                                "send_tx thread send failed: {:?}",
                                                err
                                            )),
                                        );
                                    }
                                }
                                !ui.button("Del").clicked()
                            })
                            .inner
                        })
                    })
                });
                ui.add_space(LINESPREAD);
                if ui
                    .add_sized([ui.available_width(), 20.0], Button::new("New Command"))
                    .clicked()
                {
                    self.gui_conf.commands.push(Command {
                        name: format!("Command {}", self.gui_conf.commands.len() + 1),
                        cmd: "".to_owned(),
                        editing: false,
                    })
                };
            });
    }
}

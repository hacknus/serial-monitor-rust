use crate::gui::GuiSettingsContainer;
use crate::update::{check_update, update};
use eframe::egui;
use eframe::egui::{Align2, InnerResponse, Vec2, Visuals};
use egui_theme_switch::ThemeSwitch;
use self_update::update::Release;
use semver::Version;

pub fn settings_window(
    ctx: &egui::Context,
    gui_conf: &mut GuiSettingsContainer,
    new_release: &mut Option<Release>,
    settings_window_open: &mut bool,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Settings")
        .fixed_size(Vec2 { x: 600.0, y: 200.0 })
        .anchor(Align2::CENTER_CENTER, Vec2 { x: 0.0, y: 0.0 })
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("settings").striped(true).show(ui, |ui| {
                if ui
                    .add(ThemeSwitch::new(&mut gui_conf.theme_preference))
                    .changed()
                {
                    ui.ctx().set_theme(gui_conf.theme_preference);
                };
                gui_conf.dark_mode = ui.visuals() == &Visuals::dark();

                ui.end_row();
                ui.end_row();

                if ui.button("Check for Updates").clicked() {
                    *new_release = check_update();
                }

                let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
                ui.label(format!("Current version: {}", current_version));

                ui.end_row();

                if let Some(r) = &new_release {
                    ui.label(format!("New release: {}", r.version));
                    ui.end_row();
                    if ui.button("Update").clicked() {
                        if let Ok(()) = update(r.clone()) {
                            *new_release = None;
                        }
                    }
                } else {
                    ui.label("");
                    ui.end_row();
                    ui.horizontal(|ui| {
                        ui.disable();
                        let _ = ui.button("Update");
                    });
                    ui.label("No new update");
                }

                ui.end_row();
                if ui.button("Exit Settings").clicked() {
                    *settings_window_open = false;
                }
            });
        })
}

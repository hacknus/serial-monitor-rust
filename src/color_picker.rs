use eframe::egui::{
    self, lerp, pos2, remap_clamp, vec2, Align2, Color32, Mesh, Response, Sense, Shape, Stroke, Ui,
    Vec2,
};
use eframe::epaint::StrokeKind;

// Apple system colors – Default (light) variant
pub const COLORS_LIGHT: [Color32; 12] = [
    Color32::from_rgb(58, 58, 60),    // Primary
    Color32::from_rgb(255, 56, 60),   // Red
    Color32::from_rgb(52, 199, 89),   // Green
    Color32::from_rgb(0, 136, 255),   // Blue
    Color32::from_rgb(255, 141, 40),  // Orange
    Color32::from_rgb(0, 192, 232),   // Cyan
    Color32::from_rgb(255, 204, 0),   // Yellow
    Color32::from_rgb(203, 48, 224),  // Purple
    Color32::from_rgb(172, 127, 94),  // Brown
    Color32::from_rgb(255, 45, 85),   // Pink
    Color32::from_rgb(97, 85, 245),   // Indigo
    Color32::from_rgb(0, 200, 179),   // Mint
];

// Apple system colors – Default (dark) variant
pub const COLORS_DARK: [Color32; 12] = [
    Color32::from_rgb(242, 243, 247), // Primary
    Color32::from_rgb(255, 66, 69),   // Red
    Color32::from_rgb(48, 209, 88),   // Green
    Color32::from_rgb(0, 145, 255),   // Blue
    Color32::from_rgb(255, 146, 48),  // Orange
    Color32::from_rgb(60, 211, 254),  // Cyan
    Color32::from_rgb(255, 214, 0),   // Yellow
    Color32::from_rgb(219, 52, 242),  // Purple
    Color32::from_rgb(183, 138, 102), // Brown
    Color32::from_rgb(255, 55, 95),   // Pink
    Color32::from_rgb(109, 124, 255), // Indigo
    Color32::from_rgb(0, 218, 195),   // Mint
];

/// Returns the Apple system color palette for the current theme.
pub fn get_colors(dark_mode: bool) -> &'static [Color32; 12] {
    if dark_mode {
        &COLORS_DARK
    } else {
        &COLORS_LIGHT
    }
}

fn contrast_color(color: Color32) -> Color32 {
    let intensity = (color.r() as f32 + color.g() as f32 + color.b() as f32) / 3.0 / 255.0;
    if intensity < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

pub fn color_picker_widget(
    ui: &mut Ui,
    label: &str,
    color: &mut [Color32],
    index: usize,
) -> Response {
    // Draw the square
    ui.horizontal(|ui| {
        // Define the desired square size (same as checkbox size)
        let square_size = ui.spacing().interact_size.y * 0.8;

        // Allocate a square of the same size as the checkbox
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(square_size, square_size), Sense::click());

        // Highlight stroke when hovered
        let stroke = if response.hovered() {
            Stroke::new(2.0, Color32::WHITE) // White stroke when hovered
        } else {
            Stroke::NONE // No stroke otherwise
        };

        // Draw the color square with possible hover outline
        ui.painter()
            .rect(rect, 2.0, color[index], stroke, StrokeKind::Middle);
        ui.label(label);
        response
    })
    .inner
}
pub fn color_picker_window(
    ctx: &egui::Context,
    color: &mut Color32,
    value: &mut f32,
    dark_mode: bool,
) -> bool {
    let mut save_button = false;
    let palette = get_colors(dark_mode);

    let _window_response = egui::Window::new("Color Menu")
        // .fixed_pos(Pos2 { x: 800.0, y: 450.0 })
        .fixed_size(Vec2 { x: 100.0, y: 100.0 })
        .anchor(Align2::CENTER_CENTER, Vec2 { x: 0.0, y: 0.0 })
        .collapsible(false)
        .show(ctx, |ui| {
            // We will create two horizontal rows with six squares each
            let square_size = ui.spacing().interact_size.y * 0.8;

            ui.vertical(|ui| {
                // First row (6 squares)
                ui.horizontal(|ui| {
                    for color_option in &palette[0..6] {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(square_size, square_size),
                            Sense::click(),
                        );

                        // Handle click to set selected color
                        if response.clicked() {
                            *color = *color_option;
                        }

                        // Stroke highlighting for hover
                        let stroke = if response.hovered() {
                            Stroke::new(2.0, Color32::WHITE)
                        } else {
                            Stroke::NONE
                        };

                        // Draw the color square
                        ui.painter()
                            .rect(rect, 2.0, *color_option, stroke, StrokeKind::Middle);
                    }
                });

                // Second row (6 squares)
                ui.horizontal(|ui| {
                    for color_option in &palette[6..12] {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(square_size, square_size),
                            Sense::click(),
                        );

                        // Handle click to set selected color
                        if response.clicked() {
                            *color = *color_option;
                        }

                        // Stroke highlighting for hover
                        let stroke = if response.hovered() {
                            Stroke::new(2.0, Color32::WHITE)
                        } else {
                            Stroke::NONE
                        };

                        // Draw the color square
                        ui.painter()
                            .rect(rect, 2.0, *color_option, stroke, StrokeKind::Middle);
                    }
                });

                // Now, create the 1D color bar slider below the grid
                ui.separator(); // Optional visual separator between grid and color bar
                                // Add a 1D color slider below the color grid
                let response = color_slider_1d(ui, value, |t| {
                    // Generate hue-based colors
                    let hue = t * 360.0; // Convert t from [0.0, 1.0] to [0.0, 360.0]
                    hsv_to_rgb(hue, 1.0, 1.0) // Full saturation and value
                });
                if response.clicked() || response.changed() || response.dragged() {
                    // Update the selected color based on the slider position
                    *color = hsv_to_rgb(*value * 360.0, 1.0, 1.0); // Update color
                }
                ui.add_space(5.0);
                ui.centered_and_justified(|ui| {
                    if ui.button("Exit").clicked() {
                        save_button = true;
                    }
                });
            });
        });

    save_button
}

// Function to create a 1D color slider
fn color_slider_1d(ui: &mut Ui, value: &mut f32, color_at: impl Fn(f32) -> Color32) -> Response {
    const N: usize = 100; // Number of segments

    let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);

        // Fill the color gradient
        let mut mesh = Mesh::default();
        for i in 0..=N {
            let t = i as f32 / (N as f32);
            let color = color_at(t);
            let x = lerp(rect.left()..=rect.right(), t);
            mesh.colored_vertex(pos2(x, rect.top()), color);
            mesh.colored_vertex(pos2(x, rect.bottom()), color);
            if i < N {
                mesh.add_triangle((2 * i) as u32, (2 * i + 1) as u32, (2 * i + 2) as u32);
                mesh.add_triangle((2 * i + 1) as u32, (2 * i + 2) as u32, (2 * i + 3) as u32);
            }
        }
        ui.painter().add(Shape::mesh(mesh));

        ui.painter()
            .rect_stroke(rect, 0.0, visuals.bg_stroke, StrokeKind::Middle); // outline

        // Show where the slider is at:
        let x = lerp(rect.left()..=rect.right(), *value);
        let r = rect.height() / 4.0;
        let picked_color = color_at(*value);
        ui.painter().add(Shape::convex_polygon(
            vec![
                pos2(x, rect.center().y),   // tip
                pos2(x + r, rect.bottom()), // right bottom
                pos2(x - r, rect.bottom()), // left bottom
            ],
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    response
}

// Convert HSV color to RGB
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Color32 {
    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;

    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
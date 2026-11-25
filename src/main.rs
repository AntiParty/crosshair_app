#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod guard;
mod gvas;
mod logo;
mod preview;
pub use finals_crosshair_core::model;
mod store;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Finals Crosshair")
            .with_icon(eframe::egui::IconData {
                rgba: logo::icon_rgba(64),
                width: 64,
                height: 64,
            })
            .with_inner_size([1120.0, 700.0])
            .with_min_inner_size([940.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Finals Crosshair",
        options,
        Box::new(|cc| {
            app::setup_style(&cc.egui_ctx);
            Ok(Box::new(app::App::new(&cc.egui_ctx)))
        }),
    )
}

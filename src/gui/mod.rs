pub mod app;
pub mod fonts;
pub mod theme;
pub mod viewport;
pub mod waveform;
pub mod waveform_mode;

use app::AppState;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "波形雕刻器",
        options,
        Box::new(|cc| {
            fonts::configure_fonts(&cc.egui_ctx);
            Box::new(AppState::default())
        }),
    )
}

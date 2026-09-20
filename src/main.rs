#[cfg(test)]
mod analysis;
mod app;
mod patterns;
mod rle;
mod rules;
mod simulation;
mod starts;
mod view;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        // Wider and less tall than the old 1100x720 — with the pattern
        // library and minimap both canvas overlays now (not layout-
        // consuming panels), only the top bar competes with the map for
        // space, so a wider, shorter default window leaves the map more
        // room without needing it maximized.
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1440.0, 810.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Cellular Automata",
        options,
        Box::new(|_cc| Ok(Box::new(app::App::new()))),
    )
}

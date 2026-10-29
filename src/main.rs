fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RustLoad — HTTP Load Testing")
            .with_inner_size([1520.0, 1040.0])
            .with_min_inner_size([840.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RustLoad",
        options,
        Box::new(|cc| Ok(Box::new(rustload::ui::RustLoadApp::new(cc)))),
    )
}

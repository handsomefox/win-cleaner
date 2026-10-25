//! win-cleaner: a Windows cache cleaner that only ever moves files to the
//! Recycle Bin. This binary hosts the egui interface; all cleaning logic
//! lives in `cleaner-core` and `cleaner-platform`.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod diagnostics;
mod icons;
mod presets;
mod strings;
mod theme;
mod ui;
mod viewmodel;
mod worker;

use eframe::egui;

fn main() -> eframe::Result {
    let log_path = diagnostics::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "win-cleaner starting");
    if let Some(path) = &log_path {
        tracing::info!("logging to {}", path.display());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(strings::ENGLISH.app_title)
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    let result = eframe::run_native(
        strings::ENGLISH.app_title,
        options,
        Box::new(|cc| Ok(Box::new(app::WinCleanerApp::new(cc)))),
    );
    // eframe saves the app's state before `run_native` returns, so the
    // updated copy starts from everything this one knew.
    if let Err(error) = app_update::restart_if_requested() {
        tracing::error!(%error, "could not start the updated app");
    }
    result
}

/// Where this app's releases are published.
const APP_INFO: app_update::AppInfo = app_update::AppInfo {
    owner: "handsomefox",
    repo: "win-cleaner",
    bin: "win-cleaner",
    version: env!("CARGO_PKG_VERSION"),
};

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    match image::load_from_memory(bytes) {
        Ok(decoded) => {
            let rgba = decoded.into_rgba8();
            let (width, height) = rgba.dimensions();
            egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            }
        }
        Err(err) => {
            tracing::warn!("failed to decode window icon: {err}");
            egui::IconData::default()
        }
    }
}

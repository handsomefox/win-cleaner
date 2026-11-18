//! win-cleaner: a Windows cache cleaner that only ever moves files to the
//! Recycle Bin. This binary hosts the egui interface; all cleaning logic
//! lives in `cleaner-core` and `cleaner-platform`.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod icons;
mod migrate;
mod pickers;
mod presets;
mod settings_file;
mod strings;
mod theme;
mod ui;
mod viewmodel;
mod worker;

use eframe::egui;

fn main() -> eframe::Result {
    // Every run appends to a dated log in `%LOCALAPPDATA%\win-cleaner\logs`
    // that the user can share when something goes wrong.
    let log_dir = app_diagnostics::init(
        cleaner_platform::logs_dir().as_deref(),
        "win-cleaner",
        "info",
    );
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "win-cleaner starting");
    if let Some(dir) = &log_dir {
        tracing::info!("logging to {}", dir.display());
    }

    // An elevated copy waits for the window that started it to close: that
    // window writes the settings this copy is about to read, and holds the
    // single-instance lock this copy takes next.
    if let Some(pid) = cleaner_platform::elevation::after_pid(std::env::args())
        && let Ok(executable) = std::env::current_exe()
    {
        tracing::info!(
            pid,
            "waiting for the window that started this copy to close"
        );
        cleaner_platform::elevation::wait_for_exit(pid, &executable);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(strings::ENGLISH.app_title)
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    // The guard lives only for the window, so an updated copy started below
    // finds the lock free.
    let result = {
        // A second launch brings this window forward instead of opening
        // another.
        let instance = match app_instance::acquire(APP_INFO.bin) {
            Ok(app_instance::Instance::Primary(guard)) => Some(guard),
            Ok(app_instance::Instance::Secondary) => {
                tracing::info!("another copy is running; asked it to come forward");
                return Ok(());
            }
            // Run without the guard rather than refuse to start.
            Err(error) => {
                tracing::warn!(%error, "running without the single-instance lock");
                None
            }
        };
        if start_elevated_copy() {
            return Ok(());
        }
        eframe::run_native(
            strings::ENGLISH.app_title,
            options,
            Box::new(|cc| {
                if let Some(guard) = &instance {
                    bring_forward_on_activate(guard, &cc.egui_ctx);
                }
                Ok(Box::new(app::WinCleanerApp::new(cc)))
            }),
        )
    };
    // eframe saves the app's state before `run_native` returns, so the
    // updated copy starts from everything this one knew.
    if let Err(error) = app_update::restart_if_requested() {
        tracing::error!(%error, "could not start the updated app");
    }
    result
}

/// With "always start with administrator rights" on, asks Windows to start
/// an elevated copy before any window opens, and reports whether one started.
/// The copy carries `--elevated`, so it never asks again, and waits for this
/// copy to exit before it takes the single-instance lock. A declined prompt
/// or a failure leaves this unelevated copy to run as usual.
fn start_elevated_copy() -> bool {
    use cleaner_platform::elevation::{self, ELEVATED_ARG, Elevation};
    if elevation::is_elevated()
        || std::env::args().any(|arg| arg == ELEVATED_ARG)
        || !elevation::always_elevated()
    {
        return false;
    }
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    match elevation::start_elevated(&executable, true) {
        Elevation::Started => {
            tracing::info!("started a copy with administrator rights");
            true
        }
        Elevation::Declined => {
            tracing::info!("administrator rights declined; starting without them");
            false
        }
        Elevation::Failed(error) => {
            tracing::warn!(%error, "could not start a copy with administrator rights");
            false
        }
    }
}

/// Where this app's releases are published.
const APP_INFO: app_update::AppInfo = app_update::AppInfo {
    name: strings::ENGLISH.app_title,
    owner: "handsomefox",
    repo: "win-cleaner",
    bin: "win-cleaner",
    version: env!("CARGO_PKG_VERSION"),
};

/// Restores and focuses the window whenever another launch signals.
fn bring_forward_on_activate(guard: &app_instance::Guard, ctx: &egui::Context) {
    let ctx = ctx.clone();
    let watching = guard.on_activate(move || {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    });
    if let Err(error) = watching {
        tracing::warn!(%error, "a second launch will not bring this window forward");
    }
}

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

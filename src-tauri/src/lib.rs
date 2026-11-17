//! Shuusshy desktop application entry point.

#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("Shuusshy supports Windows and macOS only (see docs/ARCHITECTURE.md).");

mod appearance;
/// Public so the hardware verification tools in `examples/` can drive the real pipeline.
pub mod audio;
mod commands;
mod install;
mod license;
// Shared with tools/license-keygen, which uses the signing-side helpers.
#[allow(dead_code)]
mod license_format;
mod release;
// Shared with tools/license-keygen (release signing).
#[allow(dead_code)]
mod release_format;
mod settings;
mod tray;
mod updates;

use std::sync::Mutex;

use tauri::{Emitter, Manager};

use audio::controller::AudioController;
use audio::devices::DeviceMonitor;
use commands::{AppState, Audio, DEVICES_CHANGED, Devices, ROUTING_CHANGED};
use settings::{Settings, SettingsStore};

const DB_FILE: &str = "shuusshy.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Elevated helper mode (renaming the virtual microphone): do the one job and exit before
    // any window, tray icon or audio starts.
    #[cfg(windows)]
    if let Some(code) = audio::devices::rename::run_helper_from_args() {
        std::process::exit(code);
    }
    install::prepare_portable_webview();
    tauri::Builder::default()
        // A second launch (Start menu, sign-in entry) just brings the running banner forward:
        // two copies would fight over the virtual cable.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_banner(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(updates::Updates::default());
            let state = init_state(app.handle());
            let settings = state.settings.lock().map(|s| s.clone()).unwrap_or_default();
            app.manage(state);
            commands::apply_window_settings(app.handle(), &settings);

            let handle = app.handle().clone();
            let monitor = DeviceMonitor::start(Box::new(move |snap| {
                let state = commands::state_for(&handle, snap);
                eprintln!(
                    "shuusshy: devices changed: {} inputs, {} outputs, mic={:?}, cable={:?}",
                    snap.inputs.len(),
                    snap.outputs.len(),
                    serde_json::to_value(&state.mic)
                        .ok()
                        .and_then(|v| v.get("state").cloned()),
                    serde_json::to_value(&state.cable)
                        .ok()
                        .and_then(|v| v.get("state").cloned()),
                );
                let _ = handle.emit(DEVICES_CHANGED, state);
                commands::reconcile_audio(&handle);
            }));
            if let Err(e) = &monitor {
                eprintln!("shuusshy: device manager unavailable: {e}");
            }
            if let Ok(m) = &monitor {
                commands::auto_select_microphone(app.handle(), &m.snapshot());
            }
            app.manage(Devices(monitor));

            let desired_handle = app.handle().clone();
            let event_handle = app.handle().clone();
            let controller = AudioController::start(
                Box::new(move || commands::desired_routing(&desired_handle)),
                Box::new(move |status| {
                    let _ = event_handle.emit(ROUTING_CHANGED, status);
                    tray::refresh(&event_handle);
                }),
                commands::processing_from(&settings),
            );
            app.manage(Audio(controller));
            // Without a tray icon the banner must never hide (it could not be brought back).
            updates::spawn_startup_check(app.handle().clone());
            let tray_ok = match tray::create(app.handle()) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("shuusshy: system tray unavailable: {e}");
                    false
                }
            };
            // The window starts hidden (tauri.conf.json). Started at sign-in, Shuusshy stays in
            // the tray; otherwise, or without a tray icon, the banner is shown.
            if !(install::launched_by_autostart() && tray_ok)
                && let Some(w) = app.get_webview_window(commands::MAIN_WINDOW)
            {
                let _ = w.show();
                let _ = w.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Keep the tray's Show/Hide label right however the banner was shown or hidden.
            if let tauri::WindowEvent::Focused(_) = event {
                tray::refresh(window.app_handle());
            }
            // Minimise / restore: let the page pause or resume its animations.
            if let tauri::WindowEvent::Resized(_) = event {
                let visible =
                    window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false);
                let _ = window.app_handle().emit(tray::BANNER_VISIBLE, visible);
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let keep = app
                    .state::<AppState>()
                    .settings
                    .lock()
                    .map(|s| s.app.keep_running_in_tray)
                    .unwrap_or(false);
                if keep && app.try_state::<tray::Tray>().is_some() {
                    api.prevent_close();
                    tray::hide_banner(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::resize_banner,
            commands::get_device_state,
            commands::refresh_devices,
            commands::select_input_device,
            commands::get_routing_status,
            commands::get_presets,
            commands::apply_preset,
            commands::list_monitor_devices,
            commands::start_monitor,
            commands::set_monitor_source,
            commands::stop_monitor,
            commands::get_meters,
            commands::set_virtual_mic_name,
            commands::get_accent_color,
            commands::get_license,
            commands::get_release_info,
            commands::check_for_updates,
            commands::download_update,
            commands::install_update,
            commands::open_releases_page,
            commands::export_settings,
            commands::import_settings,
            commands::get_start_with_windows,
            commands::set_start_with_windows,
            commands::open_vb_cable_site,
            commands::install_bundled_cable,
            commands::activate_license,
            commands::remove_license,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Shuusshy")
        .run(|app, event| {
            // macOS: clicking the Dock icon brings the banner back after it was hidden.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::show_banner(app);
            }
            if let tauri::RunEvent::Exit = event
                && let Some(audio) = app.try_state::<Audio>()
            {
                // Stop streams cleanly so the virtual microphone goes silent, not glitchy.
                audio.0.shutdown();
            }
        });
}

/// Opens the settings database in the per-user app data directory. If that fails the app still
/// starts with an in-memory store and the UI is told that settings will not persist.
fn init_state(app: &tauri::AppHandle) -> AppState {
    // Portable mode keeps settings next to the executable.
    let dir = match install::portable_data_dir() {
        Some(d) => Ok(d),
        None => app.path().app_data_dir(),
    };
    let opened = dir.as_ref().map_err(|e| e.to_string()).and_then(|d| {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        SettingsStore::open(&d.join(DB_FILE)).map_err(|e| e.to_string())
    });

    let data_dir = dir
        .map(|d| d.display().to_string())
        .unwrap_or_else(|_| "(unavailable)".into());

    let (store, persistent) = match opened {
        Ok(s) => (s, true),
        Err(e) => {
            eprintln!("shuusshy: settings database unavailable ({e}); using in-memory settings");
            (
                SettingsStore::open_in_memory().expect("in-memory SQLite must open"),
                false,
            )
        }
    };

    let settings = store.load().unwrap_or_else(|e| {
        eprintln!("shuusshy: failed to load settings ({e}); using defaults");
        Settings::default()
    });

    AppState {
        store: Mutex::new(store),
        settings: Mutex::new(settings),
        settings_persistent: persistent,
        data_dir,
    }
}

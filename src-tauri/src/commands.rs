//! IPC commands exposed to the frontend. Every argument is validated here before use.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WebviewWindow};

use crate::audio::chain::Levels;
use crate::audio::chain::Meters;
use crate::audio::controller::{
    AudioController, Desired, MonitorSource, Processing, RoutingStatus, Target,
};
use crate::audio::devices::{
    self, DeviceMonitor, DeviceState, Snapshot,
    model::{CableStatus, DeviceKind, EndpointState, MicStatus, auto_select},
};
use crate::audio::engines::EngineKind;
use crate::license::{self, LicenseStatus};
use crate::settings::model::Engine;
use crate::settings::presets::{self, PRESETS, PresetInfo};
use crate::settings::{Settings, SettingsStore};

pub const MAIN_WINDOW: &str = "main";
pub const BANNER_WIDTH: f64 = 360.0;
/// The banner can be dragged wider (its layout reflows); height always follows the content.
pub const BANNER_MIN_WIDTH: f64 = 340.0;
pub const BANNER_MAX_WIDTH: f64 = 560.0;
const BANNER_MIN_HEIGHT: f64 = 96.0;
const BANNER_MAX_HEIGHT: f64 = 640.0;
/// Event carrying a `DeviceState` whenever devices or the selection change.
pub const DEVICES_CHANGED: &str = "devices-changed";
/// Event carrying a `RoutingStatus` whenever the routing state changes.
pub const ROUTING_CHANGED: &str = "routing-changed";

pub struct AppState {
    pub store: Mutex<SettingsStore>,
    pub settings: Mutex<Settings>,
    /// False when the on-disk database could not be opened and an in-memory store is in use.
    pub settings_persistent: bool,
    pub data_dir: String,
}

/// The device monitor, or the reason it could not start.
pub struct Devices(pub Result<DeviceMonitor, String>);

/// The audio controller (pipeline owner).
pub struct Audio(pub AudioController);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    settings_persistent: bool,
    /// Running from the portable ZIP (settings stored next to the executable).
    portable: bool,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.clone(),
        settings_persistent: state.settings_persistent,
        portable: crate::install::portable_data_dir().is_some(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    settings.validate().map_err(|e| e.to_string())?;
    let saved = persist(&state, |current| {
        // Device selection is owned by the backend (select_input_device validates the device
        // is a physical microphone); never accept it from a general settings update.
        let mut next = settings;
        next.audio.input_device_id = current.audio.input_device_id.clone();
        next.audio.input_device_name = current.audio.input_device_name.clone();
        next.audio.virtual_device_id = current.audio.virtual_device_id.clone();
        // The license key is set only by activate_license (verified) / remove_license.
        next.app.license_key = current.app.license_key.clone();
        *current = next;
    })?;
    apply_window_settings(&app, &saved);
    apply_processing(&app, &saved);
    reconcile_audio(&app);
    Ok(saved)
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Settings, String> {
    let defaults = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .reset()
        .map_err(|e| e.to_string())?;
    *state.settings.lock().map_err(|e| e.to_string())? = defaults;
    apply_window_settings(&app, &Settings::default());
    if let Ok(m) = &app.state::<Devices>().0 {
        auto_select_microphone(&app, &m.snapshot());
    }
    emit_device_state(&app);
    apply_processing(&app, &Settings::default());
    reconcile_audio(&app);
    get_settings(state)
}

/// Resizes the banner to fit its content. Width is fixed; height is clamped.
#[tauri::command]
pub fn resize_banner(app: AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() {
        return Err("height must be a finite number".into());
    }
    let h = height.clamp(BANNER_MIN_HEIGHT, BANNER_MAX_HEIGHT);
    let w = main_window(&app)?;
    let scale = w.scale_factor().unwrap_or(1.0);
    let width = w
        .inner_size()
        .map(|s| s.width as f64 / scale)
        .unwrap_or(BANNER_WIDTH)
        .clamp(BANNER_MIN_WIDTH, BANNER_MAX_WIDTH);
    // Width is user-resizable within limits; height is pinned to the content. The old
    // limits are lifted first: Windows clamps a resize to whatever limits are in force.
    let _ = w.set_min_size(None::<LogicalSize<f64>>);
    let _ = w.set_max_size(None::<LogicalSize<f64>>);
    w.set_size(LogicalSize::new(width, h))
        .map_err(|e| e.to_string())?;
    let _ = w.set_min_size(Some(LogicalSize::new(BANNER_MIN_WIDTH, h)));
    let _ = w.set_max_size(Some(LogicalSize::new(BANNER_MAX_WIDTH, h)));
    Ok(())
}

#[tauri::command]
pub fn get_device_state(app: AppHandle) -> DeviceState {
    current_device_state(&app, false)
}

/// Re-enumerates devices immediately (normally this happens automatically on hot-plug).
#[tauri::command]
pub fn refresh_devices(app: AppHandle) -> DeviceState {
    let state = current_device_state(&app, true);
    let _ = app.emit(DEVICES_CHANGED, &state);
    state
}

/// Selects the physical microphone. Rejects unknown, inactive and virtual devices.
#[tauri::command]
pub fn select_input_device(
    app: AppHandle,
    state: State<'_, AppState>,
    devices: State<'_, Devices>,
    id: String,
) -> Result<DeviceState, String> {
    let monitor = devices.0.as_ref().map_err(Clone::clone)?;
    let snap = monitor.snapshot();
    let device = snap
        .inputs
        .iter()
        .find(|d| d.id == id)
        .ok_or("That microphone is not connected")?;
    if device.kind != DeviceKind::Physical {
        return Err(
            "Only physical microphones can be selected (virtual devices would loop or double-process audio)"
                .into(),
        );
    }
    let (id, name) = (device.id.clone(), device.name.clone());
    persist(&state, |s| {
        s.audio.input_device_id = Some(id);
        s.audio.input_device_name = Some(name);
    })?;
    let out = current_device_state(&app, false);
    let _ = app.emit(DEVICES_CHANGED, &out);
    reconcile_audio(&app);
    Ok(out)
}

/// Builds the device state from the latest snapshot and the stored selection.
pub fn current_device_state(app: &AppHandle, refresh: bool) -> DeviceState {
    let devices = app.state::<Devices>();
    match &devices.0 {
        Ok(m) => {
            let snap = if refresh { m.refresh() } else { m.snapshot() };
            state_for(app, &snap)
        }
        Err(e) => devices::unavailable_state(e.clone()),
    }
}

pub fn state_for(app: &AppHandle, snap: &Snapshot) -> DeviceState {
    let settings = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.audio.clone())
        .unwrap_or_default();
    devices::device_state(
        snap,
        settings.input_device_id.as_deref(),
        settings.input_device_name.as_deref(),
    )
}

pub fn emit_device_state(app: &AppHandle) {
    let _ = app.emit(DEVICES_CHANGED, current_device_state(app, false));
}

/// First run (or after reset): pick the default physical microphone so the app works out of the
/// box. Does nothing if a microphone is already selected, even if it is currently disconnected.
pub fn auto_select_microphone(app: &AppHandle, snap: &Snapshot) {
    let state = app.state::<AppState>();
    let already = state
        .settings
        .lock()
        .map(|s| s.audio.input_device_id.is_some())
        .unwrap_or(true);
    if already {
        return;
    }
    if let Some(d) = auto_select(&snap.inputs) {
        let (id, name) = (d.id.clone(), d.name.clone());
        if let Err(e) = persist(&state, |s| {
            s.audio.input_device_id = Some(id);
            s.audio.input_device_name = Some(name);
        }) {
            eprintln!("shuusshy: could not save auto-selected microphone: {e}");
        }
    }
}

/// Applies a change to the settings, validates, persists, and updates the in-memory copy.
fn persist(state: &AppState, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
    let mut next = guard.clone();
    change(&mut next);
    next.validate().map_err(|e| e.to_string())?;
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .save(&next)
        .map_err(|e| e.to_string())?;
    *guard = next.clone();
    Ok(next)
}

pub fn apply_window_settings(app: &AppHandle, settings: &Settings) {
    if let Ok(w) = main_window(app) {
        if let Err(e) = w.set_always_on_top(settings.app.always_on_top) {
            eprintln!("shuusshy: could not set always-on-top: {e}");
        }
        crate::appearance::apply_window(&w, &settings.app);
    }
    crate::tray::set_accent(app, crate::appearance::accent_rgb(settings.app.accent));
}

/// The accent colour to use, as `#rrggbb` (resolves "system" to the Windows accent).
#[tauri::command]
pub fn get_accent_color(state: State<'_, AppState>) -> Result<String, String> {
    let accent = state.settings.lock().map_err(|e| e.to_string())?.app.accent;
    Ok(crate::appearance::hex(crate::appearance::accent_rgb(
        accent,
    )))
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| "main window not found".to_string())
}

#[tauri::command]
pub fn get_routing_status(audio: State<'_, Audio>) -> RoutingStatus {
    audio.0.status()
}

/// Asks the audio controller to re-check what should be running. Safe to call before the
/// controller exists (during startup).
pub fn reconcile_audio(app: &AppHandle) {
    if let Some(a) = app.try_state::<Audio>() {
        a.0.reconcile();
    }
}

/// What the pipeline should be doing right now, from settings and live device state.
pub fn desired_routing(app: &AppHandle) -> Desired {
    let enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.audio.routing_enabled)
        .unwrap_or(false);
    if !enabled {
        return Desired::Off;
    }
    let state = current_device_state(app, false);
    if let Some(e) = state.error {
        return Desired::Waiting(format!("Audio devices unavailable: {e}"));
    }
    let mic = match state.mic {
        MicStatus::Available { device } => device,
        MicStatus::NotSelected => return Desired::Waiting("No microphone selected".into()),
        MicStatus::Error { message, .. } => {
            return Desired::Waiting(format!("Microphone unavailable: {message}"));
        }
        MicStatus::Disconnected { reason, .. } => {
            let why = match reason {
                EndpointState::Disabled => "disabled in Windows",
                EndpointState::Unplugged => "unplugged",
                _ => "disconnected",
            };
            return Desired::Waiting(format!("Microphone {why}"));
        }
    };
    let render = match state.cable {
        CableStatus::Ready { render, .. } => render,
        CableStatus::Incomplete { .. } => {
            return Desired::Waiting("VB-CABLE is incomplete".into());
        }
        CableStatus::NotInstalled => return Desired::Waiting("VB-CABLE is not installed".into()),
    };
    Desired::Run(Target {
        input_id: mic.id,
        input_name: mic.name,
        output_id: render.id,
        output_name: render.name,
    })
}

/// Noise-suppression settings for the engine, from the stored settings.
///
/// Pro features (DeepFilterNet, voice isolation) are applied only with a valid Pro license;
/// otherwise RNNoise runs and isolation stays off. The stored choice is kept for later.
pub fn processing_from(settings: &Settings) -> Processing {
    let pro = license::status_of(settings.app.license_key.as_deref()).is_pro();
    let engine = Ok(match settings.audio.engine {
        Engine::DeepFilterNet if pro => EngineKind::DeepFilterNet,
        _ => EngineKind::Rnnoise,
    });
    let a = &settings.audio;
    Processing {
        engine,
        enabled: a.suppression_enabled,
        strength: a.suppression_strength,
        levels: Levels {
            input_gain_db: a.input_gain_db,
            output_gain_db: a.output_gain_db,
            gate_enabled: a.noise_gate.enabled,
            gate_threshold_db: a.noise_gate.threshold_db,
            isolation_enabled: a.voice_isolation.enabled && pro,
            isolation_margin_db: a.voice_isolation.margin_db,
        },
    }
}

/// Pushes noise-suppression settings to the running engine (takes effect within one frame).
pub fn apply_processing(app: &AppHandle, settings: &Settings) {
    if let Some(a) = app.try_state::<Audio>() {
        a.0.set_processing(processing_from(settings));
    }
    crate::tray::refresh(app);
}

/// Switches noise cancellation on or off (`None` flips it), persists and applies it. Used by
/// the system tray; the banner uses `update_settings`.
pub fn set_suppression_enabled(app: &AppHandle, on: Option<bool>) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    let saved = persist(&state, |s| {
        s.audio.suppression_enabled = on.unwrap_or(!s.audio.suppression_enabled);
    })?;
    apply_processing(app, &saved);
    Ok(saved)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetState {
    presets: Vec<PresetInfo>,
    /// Id of the preset matching the current settings, or "custom".
    current: &'static str,
}

#[tauri::command]
pub fn get_presets(state: State<'_, AppState>) -> Result<PresetState, String> {
    let s = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(PresetState {
        presets: PRESETS.to_vec(),
        current: presets::matching(&s.audio),
    })
}

/// Applies a preset (engine, strength, gate; switches noise cancellation on) and saves it.
#[tauri::command]
pub fn apply_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Settings, String> {
    let pro = license::status_of(
        state
            .settings
            .lock()
            .map_err(|e| e.to_string())?
            .app
            .license_key
            .as_deref(),
    )
    .is_pro();
    if let Some(p) = PRESETS.iter().find(|p| p.id == id)
        && presets::needs_pro(p)
        && !pro
    {
        return Err(format!(
            "“{}” uses Shuusshy Pro features. Activate a Pro key in Settings, or choose “Low CPU Usage”.",
            p.name
        ));
    }
    let mut error = None;
    let saved = persist(&state, |s| {
        if let Err(e) = presets::apply(&mut s.audio, &id) {
            error = Some(e);
        }
    })?;
    if let Some(e) = error {
        return Err(e);
    }
    apply_processing(&app, &saved);
    Ok(saved)
}

/// A playback device the live test can monitor on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorDevice {
    id: String,
    name: String,
    is_default: bool,
    /// Name suggests loudspeakers: monitoring there can cause feedback.
    looks_like_speakers: bool,
}

/// Playback devices suitable for monitoring. VB-CABLE is excluded (it would loop the monitor
/// into the virtual microphone).
#[tauri::command]
pub fn list_monitor_devices(devices: State<'_, Devices>) -> Result<Vec<MonitorDevice>, String> {
    let m = devices.0.as_ref().map_err(Clone::clone)?;
    Ok(m.snapshot()
        .outputs
        .iter()
        .filter(|d| d.kind != DeviceKind::VbCable)
        .map(|d| {
            let lower = d.name.to_lowercase();
            MonitorDevice {
                id: d.id.clone(),
                name: d.name.clone(),
                is_default: d.is_default,
                looks_like_speakers: lower.contains("speaker")
                    && !lower.contains("headphone")
                    && !lower.contains("headset"),
            }
        })
        .collect())
}

fn parse_source(source: &str) -> Result<MonitorSource, String> {
    match source {
        "original" => Ok(MonitorSource::Original),
        "processed" => Ok(MonitorSource::Processed),
        other => Err(format!("unknown monitor source \"{other}\"")),
    }
}

/// Starts the live test's monitoring on a playback device (explicit user action only).
#[tauri::command]
pub fn start_monitor(
    audio: State<'_, Audio>,
    devices: State<'_, Devices>,
    device_id: String,
    source: String,
) -> Result<(), String> {
    let source = parse_source(&source)?;
    let m = devices.0.as_ref().map_err(Clone::clone)?;
    let snap = m.snapshot();
    let d = snap
        .outputs
        .iter()
        .find(|d| d.id == device_id)
        .ok_or("That playback device is not available")?;
    if d.kind == DeviceKind::VbCable {
        return Err("Monitoring on VB-CABLE would loop audio into the virtual microphone".into());
    }
    audio.0.monitor_start(d.id.clone(), d.name.clone(), source);
    Ok(())
}

#[tauri::command]
pub fn set_monitor_source(audio: State<'_, Audio>, source: String) -> Result<(), String> {
    audio.0.monitor_source(parse_source(&source)?);
    Ok(())
}

#[tauri::command]
pub fn stop_monitor(audio: State<'_, Audio>) {
    audio.0.monitor_stop();
}

/// Live meter levels (dBFS) while audio is flowing; `None` otherwise.
#[tauri::command]
pub fn get_meters(audio: State<'_, Audio>) -> Option<Meters> {
    audio.0.meters()
}

/// Shows the virtual microphone as "Shuusshy Microphone" in other apps (`shuusshy: true`) or
/// restores VB-CABLE's "CABLE Output". May show the Windows administrator prompt; runs on a
/// worker thread so the banner stays responsive while it waits.
#[tauri::command]
pub async fn set_virtual_mic_name(app: AppHandle, shuusshy: bool) -> Result<DeviceState, String> {
    use crate::audio::devices::model::{CABLE_CAPTURE_LABEL, SHUUSSHY_MIC_LABEL};
    let capture_id = match current_device_state(&app, false).cable {
        CableStatus::Ready { capture, .. } => capture.id,
        _ => return Err("VB-CABLE's recording device was not found".into()),
    };
    let label = if shuusshy {
        SHUUSSHY_MIC_LABEL
    } else {
        CABLE_CAPTURE_LABEL
    };
    tauri::async_runtime::spawn_blocking(move || {
        devices::rename::set_virtual_mic_label(&capture_id, label)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let state = current_device_state(&app, true);
    let _ = app.emit(DEVICES_CHANGED, state.clone());
    Ok(state)
}

#[tauri::command]
pub fn get_license(state: State<'_, AppState>) -> Result<LicenseStatus, String> {
    let s = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(license::status_of(s.app.license_key.as_deref()))
}

/// Verifies a Pro key offline and, if valid, stores it and switches Pro features on.
#[tauri::command]
pub fn activate_license(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> Result<LicenseStatus, String> {
    if key.len() > 512 {
        return Err("That key is far too long.".into());
    }
    license::verify(&key).map_err(|e| e.to_string())?;
    let normalized = key.trim().to_string();
    let saved = persist(&state, |s| s.app.license_key = Some(normalized))?;
    apply_processing(&app, &saved);
    let _ = app.emit(crate::tray::SETTINGS_CHANGED, saved.clone());
    Ok(license::status_of(saved.app.license_key.as_deref()))
}

/// Removes the stored key (Pro features switch off; free features keep working).
#[tauri::command]
pub fn remove_license(app: AppHandle, state: State<'_, AppState>) -> Result<LicenseStatus, String> {
    let saved = persist(&state, |s| s.app.license_key = None)?;
    apply_processing(&app, &saved);
    let _ = app.emit(crate::tray::SETTINGS_CHANGED, saved.clone());
    Ok(LicenseStatus::Free)
}

/// Whether Windows starts this copy of Shuusshy at sign-in. Read from Windows itself (not
/// from settings), so it is always the real state.
#[tauri::command]
pub fn get_start_with_windows() -> bool {
    crate::install::autostart_enabled()
}

#[tauri::command]
pub fn set_start_with_windows(enabled: bool) -> Result<bool, String> {
    crate::install::set_autostart(enabled)?;
    Ok(crate::install::autostart_enabled())
}

/// Opens the official VB-CABLE download page (fixed URL) in the default browser.
#[tauri::command]
pub fn open_vb_cable_site() -> Result<(), String> {
    crate::install::open_vb_cable_site()
}

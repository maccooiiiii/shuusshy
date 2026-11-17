//! Audio Device Manager (spec Part 7.1): enumeration, classification, hot-plug, status.
//!
//! Windows uses WASAPI / MMDevice with endpoint notifications; macOS uses CoreAudio (through
//! cpal) with polling. Both publish the same [`Snapshot`].

use std::collections::HashMap;

pub mod model;
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
pub mod rename;
#[cfg(windows)]
mod wasapi;

#[cfg(target_os = "macos")]
mod coreaudio;
#[cfg(target_os = "macos")]
mod poll_monitor;

use serde::Serialize;

use model::{AudioDevice, CableStatus, EndpointState, MicStatus};

#[cfg(windows)]
pub use monitor::DeviceMonitor;
#[cfg(target_os = "macos")]
pub use poll_monitor::DeviceMonitor;

/// Everything Shuusshy knows about the system's audio endpoints at one point in time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// Active capture endpoints (physical and virtual).
    pub inputs: Vec<AudioDevice>,
    /// Active render endpoints.
    pub outputs: Vec<AudioDevice>,
    /// State of every capture endpoint the system knows about, active or not.
    pub capture_states: HashMap<String, EndpointState>,
    /// Other apps currently playing into the virtual cable's playback side. They block
    /// Shuusshy and loop their sound into the virtual mic. Windows only (empty on macOS).
    pub cable_users: Vec<String>,
    /// Set when enumeration itself failed; lists may then be stale or empty.
    pub error: Option<String>,
}

/// One-off enumeration on the calling thread (initialises COM as MTA). For tools and tests; the
/// app uses [`DeviceMonitor`].
#[cfg(windows)]
pub fn enumerate_once() -> Result<Snapshot, String> {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    // S_FALSE (already initialised) is fine; RPC_E_CHANGED_MODE means an STA thread, also usable.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let api = wasapi::Wasapi::new().map_err(|e| wasapi::describe_error(&e))?;
    Ok(api.snapshot())
}

/// One-off enumeration (CoreAudio). For tools and tests; the app uses [`DeviceMonitor`].
#[cfg(target_os = "macos")]
pub fn enumerate_once() -> Result<Snapshot, String> {
    let snap = coreaudio::snapshot();
    match &snap.error {
        Some(e) if snap.inputs.is_empty() && snap.outputs.is_empty() => Err(e.clone()),
        _ => Ok(snap),
    }
}

/// Everything the UI needs about devices, derived from a snapshot plus the user's selection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    /// All active capture endpoints; the UI offers only `kind == physical` for selection.
    pub inputs: Vec<AudioDevice>,
    pub mic: MicStatus,
    pub cable: CableStatus,
    /// Other apps playing into the virtual cable (they block or echo into the virtual mic).
    pub cable_users: Vec<String>,
    /// Enumeration or device-manager error, if any.
    pub error: Option<String>,
}

pub fn device_state(
    snap: &Snapshot,
    selected_id: Option<&str>,
    selected_name: Option<&str>,
) -> DeviceState {
    DeviceState {
        inputs: snap.inputs.clone(),
        mic: model::mic_status(selected_id, selected_name, &snap.inputs, |id| {
            snap.capture_states
                .get(id)
                .copied()
                .unwrap_or(EndpointState::NotPresent)
        }),
        cable: model::cable_status(&snap.inputs, &snap.outputs),
        cable_users: snap.cable_users.clone(),
        error: snap.error.clone(),
    }
}

/// State reported when the device manager could not start at all.
pub fn unavailable_state(error: String) -> DeviceState {
    DeviceState {
        inputs: Vec::new(),
        mic: MicStatus::NotSelected,
        cable: CableStatus::NotInstalled,
        cable_users: Vec::new(),
        error: Some(error),
    }
}

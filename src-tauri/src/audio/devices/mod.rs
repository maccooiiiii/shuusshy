//! Audio Device Manager (spec Part 7.1): enumeration, classification, hot-plug, status.

pub mod model;
#[cfg(windows)]
mod monitor;
pub mod rename;
#[cfg(windows)]
mod wasapi;

use serde::Serialize;

use model::{AudioDevice, CableStatus, EndpointState, MicStatus};

#[cfg(windows)]
pub use monitor::DeviceMonitor;
#[cfg(windows)]
pub use wasapi::Snapshot;

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

#[cfg(windows)]
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

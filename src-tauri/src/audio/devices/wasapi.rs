//! WASAPI / MMDevice API access. Must be used from a thread that has initialised COM (MTA).

use std::collections::HashMap;

use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, PROPERTYKEY};
use windows::Win32::Media::Audio::{
    AUDCLNT_E_DEVICE_IN_USE, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_SERVICE_NOT_RUNNING,
    AudioSessionStateActive, DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED,
    DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED, DEVICE_STATEMASK_ALL, EDataFlow, IAudioClient,
    IAudioSessionControl2, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::{GUID, HSTRING, Interface, PWSTR};

use super::Snapshot;
use super::model::{
    AudioDevice, DeviceKind, Direction, EndpointProps, EndpointState, MixFormat, classify,
    conflicting_apps,
};

const fn pkey(d1: u32, d2: u16, d3: u16, d4: [u8; 8], pid: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID::from_values(d1, d2, d3, d4),
        pid,
    }
}
const DEVPROPS: (u32, u16, u16, [u8; 8]) = (
    0xa45c254e,
    0xdf1c,
    0x4efd,
    [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0],
);
/// PKEY_Device_FriendlyName
const PKEY_FRIENDLY_NAME: PROPERTYKEY = pkey(DEVPROPS.0, DEVPROPS.1, DEVPROPS.2, DEVPROPS.3, 14);
/// PKEY_Device_DeviceDesc
pub(super) const PKEY_DEVICE_DESC: PROPERTYKEY =
    pkey(DEVPROPS.0, DEVPROPS.1, DEVPROPS.2, DEVPROPS.3, 2);
/// PKEY_Device_EnumeratorName
const PKEY_ENUMERATOR: PROPERTYKEY = pkey(DEVPROPS.0, DEVPROPS.1, DEVPROPS.2, DEVPROPS.3, 24);
/// PKEY_DeviceInterface_FriendlyName (the adapter / driver name)
const PKEY_ADAPTER: PROPERTYKEY = pkey(
    0x026e516e,
    0xb814,
    0x414b,
    [0x83, 0xcd, 0x85, 0x6d, 0x6f, 0xef, 0x48, 0x22],
    2,
);

pub struct Wasapi {
    enumerator: IMMDeviceEnumerator,
}

impl Wasapi {
    pub fn new() -> windows::core::Result<Self> {
        let enumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        Ok(Self { enumerator })
    }

    /// True if `id` is VB-CABLE's primary recording endpoint ("CABLE Output", or after a
    /// rename "Shuusshy Microphone").
    pub fn is_primary_cable_capture(&self, id: &str) -> bool {
        unsafe { self.enumerator.GetDevice(&HSTRING::from(id)) }
            .and_then(|d| describe(&d, Direction::Capture, None))
            .map(|d| super::model::is_primary_cable_endpoint(&d))
            .unwrap_or(false)
    }

    pub fn enumerator(&self) -> &IMMDeviceEnumerator {
        &self.enumerator
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut errors = Vec::new();
        let mut take = |r: windows::core::Result<Vec<AudioDevice>>, what: &str| {
            r.unwrap_or_else(|e| {
                errors.push(format!("{what}: {}", describe_error(&e)));
                Vec::new()
            })
        };
        let inputs = take(self.list(eCapture), "recording devices");
        let outputs = take(self.list(eRender), "playback devices");
        let capture_states = self.states(eCapture).unwrap_or_else(|e| {
            errors.push(format!("device states: {}", describe_error(&e)));
            HashMap::new()
        });
        let cable_users = self.cable_users(&outputs);
        Snapshot {
            inputs,
            outputs,
            capture_states,
            cable_users,
            error: (!errors.is_empty()).then(|| errors.join("; ")),
        }
    }

    fn list(&self, flow: EDataFlow) -> windows::core::Result<Vec<AudioDevice>> {
        let direction = if flow == eCapture {
            Direction::Capture
        } else {
            Direction::Render
        };
        let default_id = unsafe { self.enumerator.GetDefaultAudioEndpoint(flow, eConsole) }
            .ok()
            .and_then(|d| device_id(&d).ok());
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            // A device can disappear between GetCount and Item; skip it rather than fail the list.
            let Ok(device) = (unsafe { collection.Item(i) }) else {
                continue;
            };
            if let Ok(d) = describe(&device, direction, default_id.as_deref()) {
                out.push(d);
            }
        }
        Ok(out)
    }

    fn states(&self, flow: EDataFlow) -> windows::core::Result<HashMap<String, EndpointState>> {
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(flow, DEVICE_STATE(DEVICE_STATEMASK_ALL))?
        };
        let count = unsafe { collection.GetCount()? };
        let mut map = HashMap::with_capacity(count as usize);
        for i in 0..count {
            let Ok(device) = (unsafe { collection.Item(i) }) else {
                continue;
            };
            if let (Ok(id), Ok(state)) = (device_id(&device), unsafe { device.GetState() }) {
                map.insert(id, map_state(state));
            }
        }
        Ok(map)
    }

    /// Names of other apps with an *active* audio session on any VB-CABLE playback endpoint.
    /// Exclusive-mode streams have no session and are not visible here.
    pub fn cable_users(&self, outputs: &[AudioDevice]) -> Vec<String> {
        let own = std::process::id();
        let mut names = Vec::new();
        for d in outputs.iter().filter(|d| d.kind == DeviceKind::VbCable) {
            names.extend(self.active_session_apps(&d.id, own));
        }
        conflicting_apps(names)
    }

    fn active_session_apps(&self, endpoint_id: &str, exclude_pid: u32) -> Vec<String> {
        let mut out = Vec::new();
        let result: windows::core::Result<()> = (|| unsafe {
            let dev = self.enumerator.GetDevice(&HSTRING::from(endpoint_id))?;
            let mgr: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None)?;
            let list = mgr.GetSessionEnumerator()?;
            for i in 0..list.GetCount()? {
                let Ok(ctl) = list.GetSession(i) else {
                    continue;
                };
                if ctl.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let pid = ctl2.GetProcessId().unwrap_or(0);
                if pid == 0 || pid == exclude_pid {
                    continue;
                }
                if let Some(name) = process_name(pid) {
                    out.push(name);
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("shuusshy: could not list sessions on {endpoint_id}: {e}");
        }
        out
    }

    /// State of a specific endpoint, including ones that are no longer listed as active.
    #[allow(dead_code)] // used by the capture manager in Milestone 3
    pub fn state_of(&self, id: &str) -> EndpointState {
        unsafe { self.enumerator.GetDevice(&HSTRING::from(id)) }
            .and_then(|d| unsafe { d.GetState() })
            .map(map_state)
            .unwrap_or(EndpointState::NotPresent)
    }
}

fn describe(
    device: &IMMDevice,
    direction: Direction,
    default_id: Option<&str>,
) -> windows::core::Result<AudioDevice> {
    let id = device_id(device)?;
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let read = |key: &PROPERTYKEY| -> Option<String> {
        let value = unsafe { store.GetValue(key) }.ok()?;
        if value.is_empty() {
            return None;
        }
        let s = value.to_string();
        (!s.is_empty()).then_some(s)
    };
    let description = read(&PKEY_DEVICE_DESC);
    let name = read(&PKEY_FRIENDLY_NAME)
        .or_else(|| description.clone())
        .unwrap_or_else(|| "Unknown device".into());
    let adapter = read(&PKEY_ADAPTER);
    let enumerator = read(&PKEY_ENUMERATOR);
    let kind = classify(&EndpointProps {
        adapter: adapter.as_deref(),
        enumerator: enumerator.as_deref(),
    });
    let (format, format_error) = match mix_format(device) {
        Ok(f) => (Some(f), None),
        Err(e) => (None, Some(describe_error(&e))),
    };
    Ok(AudioDevice {
        is_default: default_id == Some(id.as_str()),
        id,
        name,
        description,
        adapter,
        enumerator,
        direction,
        kind,
        format,
        format_error,
    })
}

/// Opens the endpoint's shared-mode audio client and reads its mix format. This proves the
/// device can actually be opened, not just that it is listed.
fn mix_format(device: &IMMDevice) -> windows::core::Result<MixFormat> {
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let ptr = client.GetMixFormat()?;
        // WAVEFORMATEX is packed; copy it out before reading fields.
        let wf: WAVEFORMATEX = std::ptr::read_unaligned(ptr);
        CoTaskMemFree(Some(ptr as *const _));
        Ok(MixFormat {
            sample_rate: wf.nSamplesPerSec,
            channels: wf.nChannels,
        })
    }
}

pub fn device_id(device: &IMMDevice) -> windows::core::Result<String> {
    unsafe {
        let raw = device.GetId()?;
        let id = raw.to_string();
        CoTaskMemFree(Some(raw.0 as *const _));
        id.map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
    }
}

fn map_state(s: DEVICE_STATE) -> EndpointState {
    match s {
        DEVICE_STATE_ACTIVE => EndpointState::Active,
        DEVICE_STATE_DISABLED => EndpointState::Disabled,
        DEVICE_STATE_NOTPRESENT => EndpointState::NotPresent,
        DEVICE_STATE_UNPLUGGED => EndpointState::Unplugged,
        _ => EndpointState::Unknown,
    }
}

/// Human-readable explanation for common audio errors, with the HRESULT kept for support.
pub fn describe_error(e: &windows::core::Error) -> String {
    let code = e.code();
    let hint = if code == AUDCLNT_E_DEVICE_IN_USE {
        "Device is in use exclusively by another application"
    } else if code == AUDCLNT_E_DEVICE_INVALIDATED {
        "Device was removed or reconfigured"
    } else if code == AUDCLNT_E_SERVICE_NOT_RUNNING {
        "Windows Audio service is not running"
    } else if code == E_ACCESSDENIED {
        "Access denied - check Windows microphone privacy settings"
    } else {
        return format!("{} (0x{:08X})", e.message().trim(), code.0 as u32);
    };
    format!("{hint} (0x{:08X})", code.0 as u32)
}

/// Executable name without extension (e.g. "Discord"), or `None` if the process cannot be
/// queried (protected system processes).
fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod hardware_tests {
    //! Run on a real machine with: `cargo test -- --ignored --nocapture`
    use super::*;
    use crate::audio::devices::model::{CableStatus, DeviceKind, cable_status};
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    #[test]
    #[ignore = "requires real audio hardware"]
    fn enumerates_real_devices() {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.unwrap();
        let api = Wasapi::new().unwrap();
        let snap = api.snapshot();
        for d in snap.inputs.iter().chain(&snap.outputs) {
            println!(
                "{:?} {:<13?} default={} {:<45} adapter={:?} enum={:?} format={:?} err={:?}",
                d.direction,
                d.kind,
                d.is_default,
                d.name,
                d.adapter,
                d.enumerator,
                d.format,
                d.format_error
            );
        }
        println!("capture states: {:?}", snap.capture_states);
        println!(
            "cable: {:?}",
            std::mem::discriminant(&cable_status(&snap.inputs, &snap.outputs))
        );
        assert!(snap.error.is_none(), "{:?}", snap.error);
        assert!(snap.inputs.iter().any(|d| d.kind == DeviceKind::Physical));
        if let CableStatus::Ready { render, capture } = cable_status(&snap.inputs, &snap.outputs) {
            println!(
                "VB-CABLE ready: write to {:?}, apps record from {:?}",
                render.name, capture.name
            );
        }
        for d in &snap.inputs {
            assert_eq!(api.state_of(&d.id), EndpointState::Active);
        }
        assert_eq!(
            api.state_of("{0.0.1.00000000}.{does-not-exist}"),
            EndpointState::NotPresent
        );
    }
}

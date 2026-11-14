//! Shows the virtual microphone as "Shuusshy Microphone" in other apps (Teams, Discord, Zoom).
//!
//! Apps list a recording device by its Windows endpoint name, which VB-CABLE sets to
//! "CABLE Output (VB-Audio Virtual Cable)". The first part is the endpoint *description*, the
//! same value Windows Sound settings lets users rename. Shuusshy changes only that description,
//! only for VB-CABLE's own recording endpoint, and only to one of two fixed names:
//! [`SHUUSSHY_MIC_LABEL`] or VB-CABLE's original [`CABLE_CAPTURE_LABEL`]. Apps then show
//! "Shuusshy Microphone (VB-Audio Virtual Cable)". The driver itself is not modified.
//!
//! Writing endpoint properties needs administrator rights. When the direct write is denied,
//! Shuusshy starts a copy of itself elevated (Windows shows its permission prompt) with
//! `--set-mic-label <endpoint id> <label>`; that copy re-checks both values, writes, and exits
//! before any window or audio starts (see [`run_helper_from_args`]).

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READWRITE,
};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::model::{CABLE_CAPTURE_LABEL, SHUUSSHY_MIC_LABEL};
use super::wasapi::{PKEY_DEVICE_DESC, Wasapi};

pub const HELPER_FLAG: &str = "--set-mic-label";
/// Used by the uninstaller: gives the virtual mic back its VB-CABLE name if Shuusshy renamed
/// it (Windows asks for permission), and does nothing otherwise.
pub const RESTORE_FLAG: &str = "--restore-mic-name";

#[derive(Debug, PartialEq)]
pub enum RenameError {
    /// Windows refused the write without administrator rights.
    NeedsElevation,
    /// The user declined the Windows permission prompt.
    Declined,
    Failed(String),
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedsElevation => write!(f, "administrator permission is needed"),
            Self::Declined => write!(
                f,
                "the Windows permission prompt was declined; nothing changed"
            ),
            Self::Failed(e) => write!(f, "{e}"),
        }
    }
}

/// Only the two fixed names are ever written.
pub fn allowed_label(label: &str) -> bool {
    label == SHUUSSHY_MIC_LABEL || label == CABLE_CAPTURE_LABEL
}

/// Endpoint IDs look like `{0.0.1.00000000}.{guid}`; anything else is rejected before it can
/// reach a command line.
pub fn valid_endpoint_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, '{' | '}' | '.' | '-'))
}

/// Sets the description of VB-CABLE's recording endpoint. Needs COM initialised (MTA).
fn write_label(id: &str, label: &str) -> Result<(), RenameError> {
    if !allowed_label(label) || !valid_endpoint_id(id) {
        return Err(RenameError::Failed("invalid name or device".into()));
    }
    let api = Wasapi::new().map_err(|e| RenameError::Failed(e.to_string()))?;
    // Re-check that the ID really is VB-CABLE's primary recording endpoint.
    if !api.is_primary_cable_capture(id) {
        return Err(RenameError::Failed(
            "that device is not VB-CABLE's recording endpoint".into(),
        ));
    }
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| RenameError::Failed(e.to_string()))?;
    let device = unsafe { enumerator.GetDevice(&HSTRING::from(id)) }
        .map_err(|e| RenameError::Failed(e.to_string()))?;
    let store = match unsafe { device.OpenPropertyStore(STGM_READWRITE) } {
        Ok(s) => s,
        Err(e) if e.code() == E_ACCESSDENIED => return Err(RenameError::NeedsElevation),
        Err(e) => return Err(RenameError::Failed(e.to_string())),
    };
    let mut wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
    // A borrowed VT_LPWSTR: SetValue copies it, and ManuallyDrop keeps PropVariantClear from
    // freeing our buffer.
    let value = ManuallyDrop::new(PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_LPWSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    pwszVal: PWSTR(wide.as_mut_ptr()),
                },
            }),
        },
    });
    let r = unsafe { store.SetValue(&PKEY_DEVICE_DESC, &*value) }
        .and_then(|_| unsafe { store.Commit() });
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.code() == E_ACCESSDENIED => Err(RenameError::NeedsElevation),
        Err(e) => Err(RenameError::Failed(e.to_string())),
    }
}

fn with_com<T>(f: impl FnOnce() -> T + Send) -> T
where
    T: Send,
{
    std::thread::scope(|s| {
        s.spawn(|| {
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            f()
        })
        .join()
        .expect("rename thread panicked")
    })
}

/// Renames the virtual microphone, asking Windows for administrator permission if needed.
/// Blocks until done (the permission prompt waits for the user): call from a worker thread.
pub fn set_virtual_mic_label(id: &str, label: &str) -> Result<(), RenameError> {
    match with_com(|| write_label(id, label)) {
        Err(RenameError::NeedsElevation) => run_elevated(id, label),
        other => other,
    }
}

fn run_elevated(id: &str, label: &str) -> Result<(), RenameError> {
    if !allowed_label(label) || !valid_endpoint_id(id) {
        return Err(RenameError::Failed("invalid name or device".into()));
    }
    let exe = std::env::current_exe().map_err(|e| RenameError::Failed(e.to_string()))?;
    let file = HSTRING::from(exe.as_os_str());
    // Both values were validated: no quotes or spaces in the ID, the label is a fixed string.
    let params = HSTRING::from(format!("{HELPER_FLAG} {id} \"{label}\""));
    let verb = HSTRING::from("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // Declining the prompt makes ShellExecuteExW fail with ERROR_CANCELLED.
    if unsafe { ShellExecuteExW(&mut info) }.is_err() {
        return Err(RenameError::Declined);
    }
    let process = info.hProcess;
    let mut code = 1u32;
    unsafe {
        if WaitForSingleObject(process, INFINITE) == WAIT_OBJECT_0 {
            let _ = GetExitCodeProcess(process, &mut code);
        }
        let _ = CloseHandle(process);
    }
    match code {
        0 => Ok(()),
        _ => Err(RenameError::Failed(format!(
            "Windows did not accept the new name (helper exit code {code})"
        ))),
    }
}

/// When started as the elevated helper, performs the rename and returns the exit code.
/// Called first thing in `run()`, before any window or audio starts.
pub fn run_helper_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == RESTORE_FLAG) {
        return Some(restore_original_name());
    }
    let at = args.iter().position(|a| a == HELPER_FLAG)?;
    let (Some(id), Some(label)) = (args.get(at + 1), args.get(at + 2)) else {
        return Some(2);
    };
    Some(match with_com(|| write_label(id, label)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("shuusshy: rename failed: {e}");
            3
        }
    })
}

/// Finds VB-CABLE's recording endpoint and, if it is called "Shuusshy Microphone", renames it
/// back to "CABLE Output". Exit code 0 also when there was nothing to do.
fn restore_original_name() -> i32 {
    let renamed = with_com(|| {
        let api = Wasapi::new().ok()?;
        match super::model::cable_status(&api.snapshot().inputs, &api.snapshot().outputs) {
            super::model::CableStatus::Ready { capture, .. }
                if capture.description.as_deref() == Some(SHUUSSHY_MIC_LABEL) =>
            {
                Some(capture.id)
            }
            _ => None,
        }
    });
    match renamed {
        None => 0,
        Some(id) => match set_virtual_mic_label(&id, CABLE_CAPTURE_LABEL) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("shuusshy: could not restore the virtual mic name: {e}");
                4
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fixed_labels_and_well_formed_ids_are_accepted() {
        assert!(allowed_label("Shuusshy Microphone"));
        assert!(allowed_label("CABLE Output"));
        assert!(!allowed_label("Anything else"));
        assert!(!allowed_label(""));
        assert!(valid_endpoint_id(
            "{0.0.1.00000000}.{bd1f7f5b-88ed-4d57-880f-f92406da35e0}"
        ));
        assert!(!valid_endpoint_id("{0.0.1.00000000}.{x\" --evil}"));
        assert!(!valid_endpoint_id("a b"));
        assert!(!valid_endpoint_id(""));
    }
}

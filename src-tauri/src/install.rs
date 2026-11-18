//! Installation-related behaviour: portable mode, start at sign-in, and the virtual cable's site.
//!
//! **Portable mode** (Windows) is switched on by a marker file named [`PORTABLE_MARKER`] next
//! to `shuusshy.exe` (the portable ZIP ships it). Then settings live in `<exe folder>\data`,
//! and WebView2's own cache goes to `<exe folder>\data\webview` instead of `%LOCALAPPDATA%`.
//!
//! **Start at sign-in** is the per-user `Run` registry entry on Windows and a LaunchAgent in
//! `~/Library/LaunchAgents` on macOS. Neither needs administrator rights.
//!
//! **Microsoft Store edition** (Windows, MSIX package): [`is_store_package`] is true. Sign-in
//! start then uses the package's StartupTask (declared in `store/AppxManifest.xml`), because a
//! packaged app's registry `Run` entry is not honoured; the Store delivers updates, so the
//! built-in updater is off (updates.rs).

use std::path::PathBuf;

pub const PORTABLE_MARKER: &str = "shuusshy.portable";
/// Command-line flag used by the sign-in entry: start hidden in the tray / menu bar.
pub const AUTOSTART_FLAG: &str = "--autostart";

/// Official download page of the virtual cable driver for this platform.
#[cfg(windows)]
pub const CABLE_DRIVER_URL: &str = "https://vb-audio.com/Cable/";
#[cfg(target_os = "macos")]
pub const CABLE_DRIVER_URL: &str = "https://existential.audio/blackhole/";

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

/// `<exe folder>\data` when the portable marker exists next to the executable.
pub fn portable_data_dir() -> Option<PathBuf> {
    let dir = exe_dir()?;
    dir.join(PORTABLE_MARKER)
        .is_file()
        .then(|| dir.join("data"))
}

/// In portable mode, points WebView2's user data folder into the portable directory.
/// Must run before the window (and any other thread) is created.
pub fn prepare_portable_webview() {
    #[cfg(windows)]
    if let Some(data) = portable_data_dir() {
        let webview = data.join("webview");
        let _ = std::fs::create_dir_all(&webview);
        // SAFETY: called first thing in `run()`, before any other thread exists.
        unsafe { std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", webview) };
    }
}

pub fn launched_by_autostart() -> bool {
    std::env::args().any(|a| a == AUTOSTART_FLAG) || platform::started_by_startup_task()
}

/// True when this copy runs from the Microsoft Store package (MSIX).
pub fn is_store_package() -> bool {
    platform::is_store_package()
}

/// Opens the official virtual cable download page. The URL is fixed: the frontend cannot make
/// Shuusshy open anything else.
pub fn open_vb_cable_site() -> Result<(), String> {
    open_url(CABLE_DRIVER_URL)
}

pub use platform::{autostart_enabled, open_url, set_autostart};

/// The official VB-CABLE setup shipped with Shuusshy (Windows installer and portable ZIP), as
/// is. `None` when this build has no bundled package (macOS, development builds without
/// `scripts/fetch-vbcable.ps1`).
pub fn bundled_cable_setup(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let setup = resource_dir?.join("vbcable").join("VBCABLE_Setup_x64.exe");
    setup.is_file().then_some(setup)
}

/// Opens VB-Audio's own VB-CABLE setup window with Windows' administrator prompt. The user
/// installs the driver there ("Install Driver"); Shuusshy detects the cable by itself.
#[cfg(windows)]
pub fn run_bundled_cable_setup(setup: &std::path::Path) -> Result<(), String> {
    platform::run_elevated(setup)
}

#[cfg(not(windows))]
pub fn run_bundled_cable_setup(_setup: &std::path::Path) -> Result<(), String> {
    Err("The bundled VB-CABLE installer is for Windows only.".into())
}

#[cfg(windows)]
mod platform {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, w};

    use super::AUTOSTART_FLAG;

    const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    /// The StartupTask declared in the Store package's manifest.
    const STARTUP_TASK_ID: &str = "ShuusshyStartup";

    /// True when the process has package identity (installed from the Microsoft Store).
    pub fn is_store_package() -> bool {
        use std::sync::OnceLock;
        use windows::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
        use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
        static PACKAGED: OnceLock<bool> = OnceLock::new();
        *PACKAGED.get_or_init(|| {
            let mut len = 0u32;
            // SAFETY: asks only for the length; no buffer is written.
            let r = unsafe { GetCurrentPackageFullName(&mut len, None) };
            r != APPMODEL_ERROR_NO_PACKAGE
        })
    }

    /// Runs WinRT calls on a thread of their own with COM initialised (multithreaded).
    fn with_winrt<T: Send + 'static>(
        f: impl FnOnce() -> windows::core::Result<T> + Send + 'static,
    ) -> Result<T, String> {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
        std::thread::spawn(move || {
            // SAFETY: balanced with CoUninitialize on this thread.
            let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            let r = f();
            if init.is_ok() {
                unsafe { CoUninitialize() };
            }
            r
        })
        .join()
        .map_err(|_| "Windows sign-in task failed".to_string())?
        .map_err(|e| e.message())
    }

    fn startup_task() -> windows::core::Result<windows::ApplicationModel::StartupTask> {
        windows::ApplicationModel::StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK_ID))?.join()
    }

    /// The Store package was started by its sign-in task.
    pub fn started_by_startup_task() -> bool {
        use windows::ApplicationModel::Activation::ActivationKind;
        use windows::ApplicationModel::AppInstance;
        if !is_store_package() {
            return false;
        }
        with_winrt(|| AppInstance::GetActivatedEventArgs()?.Kind())
            .is_ok_and(|k| k == ActivationKind::StartupTask)
    }

    fn store_autostart_enabled() -> bool {
        use windows::ApplicationModel::StartupTaskState;
        with_winrt(|| startup_task()?.State())
            .is_ok_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
    }

    fn set_store_autostart(on: bool) -> Result<(), String> {
        use windows::ApplicationModel::StartupTaskState;
        let state = with_winrt(move || {
            let task = startup_task()?;
            if on {
                task.RequestEnableAsync()?.join()
            } else {
                task.Disable()?;
                task.State()
            }
        })?;
        match state {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy if on => Ok(()),
            _ if !on => Ok(()),
            StartupTaskState::DisabledByUser => Err(
                "Start at sign-in is turned off for Shuusshy in Windows Settings → Apps → Startup. Turn it on there."
                    .into(),
            ),
            StartupTaskState::DisabledByPolicy => {
                Err("Your organisation's settings do not allow apps to start at sign-in.".into())
            }
            _ => Err("Windows did not turn on start at sign-in.".into()),
        }
    }
    const RUN_VALUE: PCWSTR = w!("Shuusshy");

    /// The command line Windows runs at sign-in.
    fn autostart_command() -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        Some(format!("\"{}\" {AUTOSTART_FLAG}", exe.display()))
    }

    fn read_run_value() -> Option<String> {
        let mut buf = vec![0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: buffer and size describe a valid writable buffer.
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if r.is_err() {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..len.min(buf.len())]))
    }

    /// True if Windows starts *this* copy of Shuusshy at sign-in.
    pub fn autostart_enabled() -> bool {
        if is_store_package() {
            return store_autostart_enabled();
        }
        read_run_value()
            .zip(autostart_command())
            .is_some_and(|(v, c)| v.eq_ignore_ascii_case(&c))
    }

    /// Adds or removes the per-user sign-in entry (HKCU\…\Run, no admin rights needed).
    pub fn set_autostart(on: bool) -> Result<(), String> {
        if is_store_package() {
            return set_store_autostart(on);
        }
        if on {
            let cmd = autostart_command().ok_or("cannot find Shuusshy's own location")?;
            let wide: Vec<u16> = cmd.encode_utf16().chain(Some(0)).collect();
            // SAFETY: data points to a NUL-terminated UTF-16 string of the given byte size.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_KEY,
                    RUN_VALUE,
                    REG_SZ.0,
                    Some(wide.as_ptr().cast()),
                    (wide.len() * 2) as u32,
                )
            }
            .ok()
            .map_err(|e| e.to_string())
        } else {
            // SAFETY: plain registry call with static strings.
            let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) };
            // Already absent is fine.
            if r.is_ok() || r.0 == 2 {
                Ok(())
            } else {
                Err(r.to_hresult().message())
            }
        }
    }

    /// Opens an https URL that Shuusshy itself chose (never one from the frontend).
    pub fn open_url(url: &str) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("refusing to open a non-https address".into());
        }
        let url = HSTRING::from(url);
        // SAFETY: all strings are valid for the call.
        let h = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &url,
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute returns a value > 32 on success.
        if h.0 as isize > 32 {
            Ok(())
        } else {
            Err("Windows could not open the browser".into())
        }
    }

    /// Starts a program with Windows' administrator prompt ("runas") and returns at once.
    pub fn run_elevated(exe: &std::path::Path) -> Result<(), String> {
        let file = HSTRING::from(exe.as_os_str());
        let dir = exe.parent().map(|d| HSTRING::from(d.as_os_str()));
        // SAFETY: all strings are valid for the call.
        let h = unsafe {
            ShellExecuteW(
                None,
                w!("runas"),
                &file,
                PCWSTR::null(),
                dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
                SW_SHOWNORMAL,
            )
        };
        match h.0 as isize {
            v if v > 32 => Ok(()),
            // SE_ERR_ACCESSDENIED: the user declined the administrator prompt.
            5 => Err(
                "The Windows permission prompt was declined; VB-CABLE was not installed.".into(),
            ),
            v => Err(format!(
                "Windows could not start the VB-CABLE installer (code {v})."
            )),
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    /// Only the Windows Store edition is packaged.
    pub fn is_store_package() -> bool {
        false
    }

    pub fn started_by_startup_task() -> bool {
        false
    }

    use std::path::PathBuf;

    use super::AUTOSTART_FLAG;

    const AGENT_LABEL: &str = "com.shuusshy.app.autostart";

    fn agent_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library/LaunchAgents")
                .join(format!("{AGENT_LABEL}.plist")),
        )
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    /// The LaunchAgent that starts this copy of Shuusshy at sign-in (hidden in the menu bar).
    pub(super) fn agent_plist(exe: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>{AUTOSTART_FLAG}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
</dict>
</plist>
"#,
            xml_escape(exe)
        )
    }

    fn exe() -> Option<String> {
        Some(std::env::current_exe().ok()?.to_str()?.to_string())
    }

    /// True if the LaunchAgent exists and starts *this* copy of Shuusshy.
    pub fn autostart_enabled() -> bool {
        let (Some(path), Some(exe)) = (agent_path(), exe()) else {
            return false;
        };
        std::fs::read_to_string(path).is_ok_and(|s| s == agent_plist(&exe))
    }

    /// Writes or removes the LaunchAgent (takes effect at the next sign-in).
    pub fn set_autostart(on: bool) -> Result<(), String> {
        let path = agent_path().ok_or("cannot find your home folder")?;
        if on {
            let exe = exe().ok_or("cannot find Shuusshy's own location")?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, agent_plist(&exe)).map_err(|e| e.to_string())
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }

    /// Opens an https URL that Shuusshy itself chose (never one from the frontend).
    pub fn open_url(url: &str) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("refusing to open a non-https address".into());
        }
        std::process::Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())
            .and_then(|s| {
                s.success()
                    .then_some(())
                    .ok_or_else(|| "macOS could not open the browser".to_string())
            })
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn launch_agent_escapes_the_path() {
            let p = super::agent_plist("/Applications/A&B <x>.app/Contents/MacOS/shuusshy");
            assert!(p.contains(
                "<string>/Applications/A&amp;B &lt;x&gt;.app/Contents/MacOS/shuusshy</string>"
            ));
            assert!(p.contains("<string>--autostart</string>"));
            assert!(p.contains("<key>RunAtLoad</key>"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bundled_cable_setup;

    #[test]
    fn finds_the_bundled_vb_cable_setup_only_where_it_exists() {
        let dir = std::env::temp_dir().join(format!("shuusshy-vbcable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(bundled_cable_setup(Some(dir.clone())), None);
        assert_eq!(bundled_cable_setup(None), None);
        std::fs::create_dir_all(dir.join("vbcable")).unwrap();
        std::fs::write(dir.join("vbcable").join("VBCABLE_Setup_x64.exe"), b"x").unwrap();
        let found = bundled_cable_setup(Some(dir.clone()));
        if cfg!(windows) {
            assert_eq!(
                found,
                Some(dir.join("vbcable").join("VBCABLE_Setup_x64.exe"))
            );
        } else {
            assert_eq!(found, None);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Installation-related behaviour: portable mode, start at sign-in, and the virtual cable's site.
//!
//! **Portable mode** (Windows) is switched on by a marker file named [`PORTABLE_MARKER`] next
//! to `shuusshy.exe` (the portable ZIP ships it). Then settings live in `<exe folder>\data`,
//! and WebView2's own cache goes to `<exe folder>\data\webview` instead of `%LOCALAPPDATA%`.
//!
//! **Start at sign-in** is the per-user `Run` registry entry on Windows and a LaunchAgent in
//! `~/Library/LaunchAgents` on macOS. Neither needs administrator rights.

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
    std::env::args().any(|a| a == AUTOSTART_FLAG)
}

/// Opens the official virtual cable download page. The URL is fixed: the frontend cannot make
/// Shuusshy open anything else.
pub fn open_vb_cable_site() -> Result<(), String> {
    open_url(CABLE_DRIVER_URL)
}

pub use platform::{autostart_enabled, open_url, set_autostart};

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
        read_run_value()
            .zip(autostart_command())
            .is_some_and(|(v, c)| v.eq_ignore_ascii_case(&c))
    }

    /// Adds or removes the per-user sign-in entry (HKCU\…\Run, no admin rights needed).
    pub fn set_autostart(on: bool) -> Result<(), String> {
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
}

#[cfg(target_os = "macos")]
mod platform {
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

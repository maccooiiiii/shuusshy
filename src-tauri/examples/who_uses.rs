//! Read-only diagnostic: lists the processes with audio sessions on every active endpoint, to
//! find what is holding a device (e.g. "CABLE Input" in exclusive mode). Changes nothing.
//!
//! ```text
//! cargo run --release --example who_uses
//! ```

use shuusshy_lib::audio::devices::enumerate_once;
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, AudioSessionStateExpired, IAudioSessionControl2,
    IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{HSTRING, Interface};

fn main() {
    let snap = enumerate_once().expect("enumerate");
    let en: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.unwrap();
    for d in snap.inputs.iter().chain(&snap.outputs) {
        println!("{} [{:?}]", d.name, d.direction);
        unsafe {
            let dev = en.GetDevice(&HSTRING::from(&d.id)).unwrap();
            let mgr: IAudioSessionManager2 = match dev.Activate(CLSCTX_ALL, None) {
                Ok(m) => m,
                Err(e) => {
                    println!("  (cannot query sessions: {e})");
                    continue;
                }
            };
            let list = mgr.GetSessionEnumerator().unwrap();
            for i in 0..list.GetCount().unwrap_or(0) {
                let Ok(ctl) = list.GetSession(i) else {
                    continue;
                };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let pid = ctl2.GetProcessId().unwrap_or(0);
                let state = ctl.GetState().unwrap_or(AudioSessionStateExpired);
                let name = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).Path"),
                    ])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default();
                let state = if state == AudioSessionStateActive {
                    "ACTIVE"
                } else {
                    "inactive"
                };
                println!(
                    "  pid {pid:<6} {state:<8} {}",
                    if pid == 0 { "(system sounds)" } else { &name }
                );
            }
        }
    }
}

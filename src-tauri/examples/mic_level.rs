//! Read-only diagnostic: prints each active recording endpoint's mute state and volume, and
//! measures its actual signal level for a few seconds. Changes nothing.
//!
//! ```text
//! cargo run --release --example mic_level -- [seconds=4]
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, SampleFormat};
use shuusshy_lib::audio::devices::enumerate_once;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::HSTRING;

fn main() {
    let secs: f64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4.0);
    let snap = enumerate_once().expect("enumerate");
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .ok();
    let en: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.unwrap();
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();

    for d in &snap.inputs {
        let (mute, vol, db) = unsafe {
            let dev = en.GetDevice(&HSTRING::from(&d.id)).unwrap();
            let ev: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).unwrap();
            (
                ev.GetMute().map(|b| b.as_bool()).ok(),
                ev.GetMasterVolumeLevelScalar().ok(),
                ev.GetMasterVolumeLevel().ok(),
            )
        };
        println!(
            "{} [{:?}]\n  muted={mute:?} volume={:.0}% ({:.1} dB)",
            d.name,
            d.kind,
            vol.unwrap_or(f32::NAN) * 100.0,
            db.unwrap_or(f32::NAN)
        );

        let dev = host
            .device_by_id(&DeviceId::new(HostId::Wasapi, &d.id))
            .unwrap();
        let cfg = dev.default_input_config().unwrap();
        if cfg.sample_format() != SampleFormat::F32 {
            println!("  (format {:?}, level not measured)", cfg.sample_format());
            continue;
        }
        let acc = Arc::new(Mutex::new((0.0f64, 0u64, 0.0f32, 0u64)));
        let a = acc.clone();
        let stream = dev
            .build_input_stream(
                cfg.config(),
                move |data: &[f32], _| {
                    let mut a = a.lock().unwrap();
                    for &s in data {
                        a.0 += (s as f64) * (s as f64);
                        a.1 += 1;
                        a.2 = a.2.max(s.abs());
                        if s == 0.0 {
                            a.3 += 1;
                        }
                    }
                },
                |e| eprintln!("  stream error: {e}"),
                None,
            )
            .unwrap();
        stream.play().unwrap();
        std::thread::sleep(Duration::from_secs_f64(secs));
        drop(stream);
        let (sum, n, peak, zeros) = *acc.lock().unwrap();
        let rms = 10.0 * (sum / n.max(1) as f64).max(1e-20).log10();
        println!(
            "  measured over {secs} s: RMS {rms:.1} dBFS, peak {:.1} dBFS, exact zeros {:.1}%",
            20.0 * (peak as f64).max(1e-10).log10(),
            100.0 * zeros as f64 / n.max(1) as f64
        );
    }
}

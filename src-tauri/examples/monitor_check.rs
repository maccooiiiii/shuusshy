//! Hardware verification for Milestone 7's headphone monitoring.
//!
//! While the Shuusshy app is monitoring (TEST → Hear original / Hear processed), this records
//! the microphone and, via WASAPI loopback, what the monitor device is actually playing, and
//! compares them: level, correlation with the microphone, and delay. Read-only: it plays
//! nothing and changes nothing.
//!
//! ```text
//! cargo run --release --example monitor_check -- "<part of the monitor device name>" [seconds=6]
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, SampleFormat};
use shuusshy_lib::audio::devices::{enumerate_once, model::auto_select};

type Rec = Arc<Mutex<(Vec<f32>, Option<i128>)>>;

/// Records `device` as input (a playback device is recorded in loopback mode), mono, 48 kHz.
fn record(device: &cpal::Device, loopback: bool, cap: usize) -> (cpal::Stream, Rec) {
    let cfg = if loopback {
        device.default_output_config().unwrap()
    } else {
        device.default_input_config().unwrap()
    };
    assert_eq!(cfg.sample_format(), SampleFormat::F32);
    assert_eq!(cfg.sample_rate(), 48_000, "analysis assumes 48 kHz");
    let ch = cfg.channels() as usize;
    let rec: Rec = Arc::new(Mutex::new((Vec::with_capacity(cap), None)));
    let r = rec.clone();
    let stream = device
        .build_input_stream(
            cfg.config(),
            move |data: &[f32], info: &cpal::InputCallbackInfo| {
                let mut r = r.lock().unwrap();
                if r.1.is_none() {
                    r.1 = Some(info.timestamp().capture.as_nanos() as i128);
                }
                for f in data.chunks_exact(ch) {
                    if r.0.len() < r.0.capacity() {
                        r.0.push(f.iter().sum::<f32>() / ch as f32);
                    }
                }
            },
            |e| eprintln!("capture error: {e}"),
            None,
        )
        .unwrap();
    (stream, rec)
}

fn rms_db(x: &[f32]) -> f64 {
    let ms = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * ms.max(1e-20).log10()
}

/// Best normalized correlation of block envelopes (20 ms RMS in dB), robust to the different
/// clocks of the two devices and to noise suppression changing the waveform.
fn envelope_match(a: &[f32], b: &[f32]) -> (f64, isize) {
    let env = |x: &[f32]| -> Vec<f64> { x.chunks(960).map(|c| rms_db(c).max(-90.0)).collect() };
    let (ea, eb) = (env(a), env(b));
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let (ma, mb) = (mean(&ea), mean(&eb));
    let mut best = (f64::MIN, 0isize);
    for lag in 0..25isize {
        // lag in 20 ms blocks: b is a delayed version of a
        let n = ea.len().min(eb.len()).saturating_sub(lag as usize);
        let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
        for i in 0..n {
            let x = ea[i] - ma;
            let y = eb[i + lag as usize] - mb;
            num += x * y;
            da += x * x;
            db += y * y;
        }
        let r = num / (da * db).sqrt().max(1e-12);
        if r > best.0 {
            best = (r, lag);
        }
    }
    best
}

fn main() {
    let name = std::env::args()
        .nth(1)
        .expect("usage: monitor_check <device name part> [seconds]");
    let secs: f64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(6.0);
    let snap = enumerate_once().expect("enumerate");
    let mic = auto_select(&snap.inputs).expect("no microphone").clone();
    let out = snap
        .outputs
        .iter()
        .find(|d| d.name.to_lowercase().contains(&name.to_lowercase()))
        .unwrap_or_else(|| panic!("no playback device matching \"{name}\""))
        .clone();
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();
    let mic_dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, &mic.id))
        .unwrap();
    let out_dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, &out.id))
        .unwrap();
    println!("mic: {}\nmonitor device (loopback): {}", mic.name, out.name);

    let cap = (48_000.0 * (secs + 1.0)) as usize;
    let (sm, rm) = record(&mic_dev, false, cap);
    let (so, ro) = record(&out_dev, true, cap);
    sm.play().unwrap();
    so.play().unwrap();
    std::thread::sleep(Duration::from_secs_f64(secs));
    drop(sm);
    drop(so);
    let m = rm.lock().unwrap().0.clone();
    let o = ro.lock().unwrap().0.clone();
    let (r, lag) = envelope_match(&m, &o);
    println!(
        "mic level {:.1} dBFS, monitor playback level {:.1} dBFS",
        rms_db(&m),
        rms_db(&o)
    );
    println!(
        "loudness-envelope correlation with the mic: {r:.2} at a delay of {} ms",
        lag * 20
    );
}

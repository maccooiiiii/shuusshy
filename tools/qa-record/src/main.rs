//! QA tool: records the physical microphone and the virtual microphone ("CABLE Output" /
//! "Shuusshy Microphone") simultaneously, mono 48 kHz float WAV, and prints level statistics.
//! Recordings are only made when someone runs this tool on purpose.
//!
//! cargo run --release -- <out_dir> [seconds=10]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

type Buf = Arc<Mutex<Vec<f32>>>;
/// Raw first two channels of the microphone, for a phase check (L vs R).
type Raw = Arc<Mutex<(Vec<f32>, Vec<f32>)>>;

fn open(dev: &cpal::Device, cap: usize, raw: Option<Raw>) -> (cpal::Stream, Buf, u32) {
    let cfg = dev.default_input_config().expect("input config");
    let ch = cfg.channels() as usize;
    let rate = cfg.sample_rate();
    let buf: Buf = Arc::new(Mutex::new(Vec::with_capacity(cap)));
    let b = buf.clone();
    let s = dev
        .build_input_stream(
            cfg.config(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut b = b.lock().unwrap();
                for f in data.chunks_exact(ch) {
                    if b.len() < b.capacity() {
                        b.push(f.iter().sum::<f32>() / ch as f32);
                        if let Some(r) = &raw
                            && ch >= 2
                        {
                            let mut r = r.lock().unwrap();
                            r.0.push(f[0]);
                            r.1.push(f[1]);
                        }
                    }
                }
            },
            |e| eprintln!("stream error: {e}"),
            None,
        )
        .expect("open stream");
    (s, buf, rate)
}

fn stats(name: &str, x: &[f32]) {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    let rms = (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt();
    let near_full = x.iter().filter(|v| v.abs() >= 0.97).count();
    let db = |v: f64| 20.0 * v.max(1e-10).log10();
    println!(
        "{name:<10} peak {:6.1} dBFS  rms {:6.1} dBFS  samples >= -0.26 dBFS: {near_full}  zeros {:.1}%",
        db(peak as f64),
        db(rms),
        100.0 * x.iter().filter(|v| **v == 0.0).count() as f64 / x.len().max(1) as f64
    );
}

fn save(path: &std::path::Path, x: &[f32], rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    for v in x {
        w.write_sample(*v).unwrap();
    }
    w.finalize().unwrap();
    println!("wrote {}", path.display());
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: <out_dir> [seconds]"));
    let secs: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    std::fs::create_dir_all(&out).unwrap();
    let host = cpal::host_from_id(cpal::HostId::Wasapi).unwrap();
    let mut mic = None;
    let mut cable = None;
    for d in host.input_devices().unwrap() {
        let n = d.description().map(|x| x.name().to_string()).unwrap_or_default();
        let lower = n.to_lowercase();
        if lower.contains("vb-audio") || lower.contains("cable output") || lower.contains("shuusshy") {
            cable.get_or_insert((n, d));
        } else if lower.contains("realtek") || lower.contains("microphone") {
            mic.get_or_insert((n, d));
        }
    }
    let (mn, md) = mic.expect("no microphone");
    let (cn, cd) = cable.expect("no virtual microphone");
    println!("mic: {mn}\nvirtual mic: {cn}\nrecording {secs} s…");
    let cap = (48_000.0 * (secs + 1.0)) as usize;
    let raw: Raw = Arc::new(Mutex::new((Vec::new(), Vec::new())));
    let (s1, b1, r1) = open(&md, cap, Some(raw.clone()));
    let (s2, b2, r2) = open(&cd, cap, None);
    s1.play().unwrap();
    s2.play().unwrap();
    std::thread::sleep(Duration::from_secs_f64(secs));
    drop(s1);
    drop(s2);
    let m = b1.lock().unwrap().clone();
    let c = b2.lock().unwrap().clone();
    stats("mic", &m);
    {
        let r = raw.lock().unwrap();
        let (l, rr) = (&r.0, &r.1);
        if !l.is_empty() {
            let e = |x: &[f32]| x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
            let dot: f64 = l.iter().zip(rr).map(|(a, b)| *a as f64 * *b as f64).sum();
            let corr = dot / (e(l) * e(rr)).sqrt().max(1e-12);
            let sum: Vec<f32> = l.iter().zip(rr).map(|(a, b)| (a + b) / 2.0).collect();
            let db = |v: f64| 10.0 * v.max(1e-20).log10();
            println!(
                "mic channels: L {:.1} dBFS, R {:.1} dBFS, (L+R)/2 {:.1} dBFS, L/R correlation {:+.2}",
                db(e(l) / l.len() as f64),
                db(e(rr) / rr.len() as f64),
                db(e(&sum) / sum.len() as f64),
                corr
            );
            save(&out.join("0-mic-left.wav"), l, r1);
            save(&out.join("0-mic-right.wav"), rr, r1);
        }
    }
    stats("virtual", &c);
    save(&out.join("1-mic-original.wav"), &m, r1);
    save(&out.join("2-virtual-mic-processed.wav"), &c, r2);
}

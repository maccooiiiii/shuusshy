//! Offline engine comparison with a known clean reference.
//!
//! Takes clean 48 kHz mono 16-bit speech WAV files (e.g. generated with Windows text-to-speech),
//! adds reproducible "fan" noise and keyboard clicks, runs each engine/strength through the
//! real Shuusshy processors, and compares every output with the clean speech:
//!
//! - **speech level**: output vs clean, in blocks where the clean speech is active;
//! - **noise in pauses**: output level where the clean speech is silent;
//! - **SI-SDR** (scale-invariant signal-to-distortion ratio, dB, higher is better): how close the
//!   output is to the clean speech, ignoring overall gain;
//! - **gain**: the overall scale of the speech in the output (dB).
//!
//! ```text
//! cargo run --release --example engine_compare -- speech1.wav [speech2.wav ...]
//! ```

use std::sync::Arc;

use shuusshy_lib::audio::engines::{
    EngineShared,
    deepfilter::{self, DeepFilterProcessor},
    rnnoise::RnnoiseProcessor,
};
use shuusshy_lib::audio::pipeline::{FRAME, FrameProcessor};

fn read_wav(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).expect("read wav");
    assert_eq!(&b[0..4], b"RIFF", "not a WAV file");
    let mut i = 12;
    let (mut rate, mut bits, mut ch) = (0u32, 0u16, 0u16);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " {
            ch = u16::from_le_bytes(body[2..4].try_into().unwrap());
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            assert!(
                rate == 48_000 && bits == 16 && ch == 1,
                "need 48 kHz 16-bit mono, got {rate}/{bits}/{ch}"
            );
            return body
                .as_chunks::<2>()
                .0
                .iter()
                .map(|s| i16::from_le_bytes(*s) as f32 / 32768.0)
                .collect();
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

fn rng(seed: u64) -> impl FnMut() -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    move || {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}
fn db(v: f64) -> f64 {
    20.0 * v.max(1e-10).log10()
}

/// Fan-like noise (low-passed) at about -38 dBFS plus keyboard clicks.
fn noise(len: usize, seed: u64) -> Vec<f32> {
    let mut r = rng(seed);
    let (mut a, mut b) = (0.0f32, 0.0f32);
    let mut n: Vec<f32> = (0..len)
        .map(|_| {
            a += 0.08 * (r() - a);
            b += 0.08 * (a - b);
            b * 0.25 + r() * 0.002
        })
        .collect();
    // Clicks: short decaying bursts every 120-420 ms.
    let mut i = 4_800;
    while i + 2_000 < len {
        let amp = 0.15 + 0.15 * r().abs();
        for k in 0..1_500 {
            n[i + k] += amp * r() * (-(k as f32) / 180.0).exp();
        }
        i += 5_760 + ((r().abs() * 14_400.0) as usize);
    }
    n
}

fn run(p: &mut dyn FrameProcessor, input: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(input.len());
    for chunk in input.as_chunks::<FRAME>().0 {
        let mut f = *chunk;
        p.process(&mut f);
        out.extend_from_slice(&f);
    }
    out
}

struct Score {
    speech_change_db: f64,
    pause_db: f64,
    si_sdr_db: f64,
    gain_db: f64,
}

/// Compares `out` (delayed by `delay`) with `clean` over the region after a 1 s warm-up.
fn score(clean: &[f32], out: &[f32], delay: usize) -> Score {
    let start = 48_000;
    let end = clean.len().min(out.len() - delay) / 960 * 960;
    let c = &clean[start..end];
    let o = &out[start + delay..end + delay];
    // Active blocks: clean 20 ms block within 30 dB of the loudest block.
    let blocks: Vec<f64> = c.chunks(960).map(rms).collect();
    let peak = blocks.iter().cloned().fold(0.0, f64::max);
    let active: Vec<bool> = blocks.iter().map(|&b| db(b) > db(peak) - 30.0).collect();
    let silent: Vec<bool> = blocks.iter().map(|&b| db(b) < db(peak) - 50.0).collect();
    let pick = |x: &[f32], mask: &[bool]| -> Vec<f32> {
        x.chunks(960)
            .zip(mask)
            .filter(|(_, m)| **m)
            .flat_map(|(b, _)| b.iter().copied())
            .collect()
    };
    let speech_change_db = db(rms(&pick(o, &active))) - db(rms(&pick(c, &active)));
    let pause_db = db(rms(&pick(o, &silent)));
    // SI-SDR with optimal scaling alpha = <o,c>/<c,c>.
    let oc: f64 = o.iter().zip(c).map(|(a, b)| *a as f64 * *b as f64).sum();
    let cc: f64 = c.iter().map(|v| (*v as f64).powi(2)).sum();
    let alpha = oc / cc;
    let (mut target, mut err) = (0.0f64, 0.0f64);
    for (a, b) in o.iter().zip(c) {
        let t = alpha * *b as f64;
        target += t * t;
        err += (*a as f64 - t).powi(2);
    }
    Score {
        speech_change_db,
        pause_db,
        si_sdr_db: 10.0 * (target / err.max(1e-20)).log10(),
        gain_db: db(alpha.abs()),
    }
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    assert!(!files.is_empty(), "usage: engine_compare <clean.wav>...");
    println!(
        "{:<26} {:>14} {:>15} {:>10} {:>9}",
        "input / engine", "speech level", "noise (pauses)", "SI-SDR", "gain"
    );
    for (n, path) in files.iter().enumerate() {
        let mut clean = read_wav(path);
        // Normalize speech to about -20 dBFS over its active part.
        let level = rms(&clean
            .iter()
            .copied()
            .filter(|v| v.abs() > 0.01)
            .collect::<Vec<_>>());
        let g = 0.1 / level as f32;
        clean.iter_mut().for_each(|v| *v *= g);
        let nz = noise(clean.len(), 42 + n as u64);
        let noisy: Vec<f32> = clean.iter().zip(&nz).map(|(s, z)| s + z).collect();
        let name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy();
        println!(
            "--- {name} ({:.1} s, noise {:.1} dBFS)",
            clean.len() as f64 / 48_000.0,
            db(rms(&nz))
        );

        let print = |label: &str, s: Score| {
            println!(
                "{:<26} {:>+12.1} dB {:>12.1} dBFS {:>7.1} dB {:>+6.1} dB",
                label, s.speech_change_db, s.pause_db, s.si_sdr_db, s.gain_db
            );
        };
        print("noisy input (reference)", score(&clean, &noisy, 0));
        for strength in [0.8f32, 1.0] {
            let mut rn = RnnoiseProcessor::new(Arc::new(EngineShared::new(true, strength)));
            print(
                &format!("RNNoise {:.0}%", strength * 100.0),
                score(&clean, &run(&mut rn, &noisy), FRAME),
            );
            let mut df = DeepFilterProcessor::with_watchdog(
                Arc::new(EngineShared::new(true, strength)),
                None,
            );
            print(
                &format!("DeepFilterNet {:.0}%", strength * 100.0),
                score(&clean, &run(&mut df, &noisy), deepfilter::DELAY),
            );
        }
    }
}

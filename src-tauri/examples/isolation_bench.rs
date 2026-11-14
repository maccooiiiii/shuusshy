//! Noise + background-voice isolation benchmark (offline, known clean reference).
//!
//! Mixes a target voice (with long pauses) with a quieter background talker, fan-like noise
//! and keyboard clicks, runs candidate processing configurations, and measures against the
//! clean target:
//!
//! - **SI-SDR**: scale-invariant signal-to-distortion ratio vs the clean target (higher =
//!   output closer to the target voice alone);
//! - **voice**: level change of the target voice where it is active;
//! - **bg leak**: output level vs the background talker's input level, where only the
//!   background talker is active (more negative = better voice isolation);
//! - **noise**: output level where nobody talks.
//!
//! ```text
//! cargo run --release --example isolation_bench -- target.wav background.wav
//! ```

use std::sync::Arc;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use shuusshy_lib::audio::chain::{ChainShared, Levels, ProcessingChain};
use shuusshy_lib::audio::engines::{
    EngineShared,
    deepfilter::{self, DeepFilterProcessor},
    rnnoise::RnnoiseProcessor,
};
use shuusshy_lib::audio::pipeline::{FRAME, FrameProcessor};

fn read_wav(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).expect("read wav");
    let mut i = 12;
    while i + 8 <= b.len() {
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        if &b[i..i + 4] == b"data" {
            return b[i + 8..(i + 8 + len).min(b.len())]
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
fn active_rms(x: &[f32]) -> f64 {
    let blocks: Vec<f64> = x.chunks(960).map(rms).collect();
    let peak = blocks.iter().cloned().fold(0.0, f64::max);
    let act: Vec<f64> = blocks
        .into_iter()
        .filter(|&b| db(b) > db(peak) - 30.0)
        .collect();
    (act.iter().map(|v| v * v).sum::<f64>() / act.len() as f64).sqrt()
}

/// Fan-like noise plus keyboard clicks.
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

fn resample(x: &[f32], from: usize, to: usize) -> Vec<f32> {
    let mut r = Fft::<f32>::new(from, to, 1024, 1, FixedSync::Input).unwrap();
    let n = r.process_all_needed_output_len(x.len());
    let mut out = vec![0.0f32; n];
    let input = InterleavedSlice::new(x, 1, x.len()).unwrap();
    let mut output = InterleavedSlice::new_mut(&mut out, 1, n).unwrap();
    let (_, written) = r
        .process_all_into_buffer(&input, &mut output, x.len(), None)
        .unwrap();
    out.truncate(written);
    out
}

/// hush-vani at its native 16 kHz: 48k → 16k → hush → 48k (all delays removed).
fn hush(x: &[f32], hush: &hush_vani::Hush) -> Vec<f32> {
    let low = resample(x, 48_000, 16_000);
    let clean = hush.enhance(&low).unwrap();
    let aligned = &clean[hush_vani::Hush::LATENCY_SAMPLES..];
    let mut up = resample(aligned, 16_000, 48_000);
    up.resize(x.len(), 0.0);
    up
}

struct Masks {
    target: Vec<bool>,
    bg_only: Vec<bool>,
    silent: Vec<bool>,
}

fn masks(target: &[f32], bg: &[f32]) -> Masks {
    let tb: Vec<f64> = target.chunks(960).map(rms).collect();
    let bb: Vec<f64> = bg.chunks(960).map(rms).collect();
    let tp = tb.iter().cloned().fold(0.0, f64::max);
    let bp = bb.iter().cloned().fold(0.0, f64::max);
    let t_on: Vec<bool> = tb.iter().map(|&v| db(v) > db(tp) - 35.0).collect();
    let t_off: Vec<bool> = tb.iter().map(|&v| db(v) < db(tp) - 55.0).collect();
    let b_on: Vec<bool> = bb.iter().map(|&v| db(v) > db(bp) - 35.0).collect();
    let b_off: Vec<bool> = bb.iter().map(|&v| db(v) < db(bp) - 55.0).collect();
    Masks {
        target: t_on,
        bg_only: t_off.iter().zip(&b_on).map(|(a, b)| *a && *b).collect(),
        silent: t_off.iter().zip(&b_off).map(|(a, b)| *a && *b).collect(),
    }
}

fn pick(x: &[f32], mask: &[bool]) -> Vec<f32> {
    x.chunks(960)
        .zip(mask)
        .filter(|(_, m)| **m)
        .flat_map(|(b, _)| b.iter().copied())
        .collect()
}

/// Scores `out` (aligned by `delay`) against the clean target.
fn score(target: &[f32], bg: &[f32], out: &[f32], delay: usize, m: &Masks) -> [f64; 4] {
    let start = 2 * 48_000; // skip warm-up
    let end = (target.len().min(out.len() - delay)) / 960 * 960;
    let t = &target[start..end];
    let b = &bg[start..end];
    let o = &out[start + delay..end + delay];
    let k0 = start / 960;
    let k1 = end / 960;
    let (mt, mb, ms) = (&m.target[k0..k1], &m.bg_only[k0..k1], &m.silent[k0..k1]);
    let oc: f64 = o.iter().zip(t).map(|(a, c)| *a as f64 * *c as f64).sum();
    let cc: f64 = t.iter().map(|v| (*v as f64).powi(2)).sum();
    let alpha = oc / cc;
    let (mut tg, mut er) = (0.0f64, 0.0f64);
    for (a, c) in o.iter().zip(t) {
        let x = alpha * *c as f64;
        tg += x * x;
        er += (*a as f64 - x).powi(2);
    }
    let si_sdr = 10.0 * (tg / er.max(1e-20)).log10();
    let voice = db(rms(&pick(o, mt))) - db(rms(&pick(t, mt)));
    let leak = db(rms(&pick(o, mb))) - db(rms(&pick(b, mb)));
    let noise = db(rms(&pick(o, ms)));
    [si_sdr, voice, leak, noise]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(
        args.len() == 2,
        "usage: isolation_bench <target.wav> <background.wav>"
    );
    let mut target = read_wav(&args[0]);
    let mut bg_raw = read_wav(&args[1]);
    let len = target.len().min(bg_raw.len());
    target.truncate(len);
    bg_raw.truncate(len);
    let tg = 0.1 / active_rms(&target) as f32; // target voice at -20 dBFS while talking
    target.iter_mut().for_each(|v| *v *= tg);
    let nz = noise(len, 7);
    let hush_model = hush_vani::Hush::new().unwrap();
    println!(
        "hush-vani AVX2: {}; target voice -20 dBFS; noise {:.1} dBFS\n",
        hush_vani::Hush::is_accelerated(),
        db(rms(&nz))
    );

    // Scenarios: (background level vs voice, voice loudness swing in dB).
    let steady = target.clone();
    for (bg_rel_db, swing) in [(-12.0f64, 0.0f32), (-6.0, 0.0), (-12.0, 5.0), (-9.0, 5.0)] {
        // A speaker who gets louder and softer: ±swing dB over an 11 s cycle.
        let target: Vec<f32> = steady
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let t = i as f32 / 48_000.0;
                v * 10f32.powf(swing * (2.0 * std::f32::consts::PI * t / 11.0).sin() / 20.0)
            })
            .collect();
        let g = (0.1 * 10f64.powf(bg_rel_db / 20.0)) as f32 / active_rms(&bg_raw) as f32;
        let bg: Vec<f32> = bg_raw.iter().map(|v| v * g).collect();
        let noisy: Vec<f32> = (0..len).map(|i| target[i] + bg[i] + nz[i]).collect();
        let m = masks(&target, &bg);
        println!(
            "=== background talker {bg_rel_db:+.0} dB vs the voice; voice loudness swing ±{swing:.0} dB ==="
        );
        println!(
            "{:<38} {:>8} {:>9} {:>10} {:>12}",
            "configuration", "SI-SDR", "voice", "bg leak", "noise floor"
        );
        let show = |name: &str, out: &[f32], delay: usize| {
            let [s, v, l, n] = score(&target, &bg, out, delay, &m);
            println!("{name:<38} {s:>6.1}dB {v:>+7.1}dB {l:>+8.1}dB {n:>8.1}dBFS");
        };
        let shared = |on: bool, s: f32| Arc::new(EngineShared::new(on, s));
        let dfn = |s: f32| DeepFilterProcessor::with_watchdog(shared(true, s), None);
        let rnn = |s: f32| RnnoiseProcessor::new(shared(true, s));

        show("noisy input (no processing)", &noisy, 0);
        show(
            "DeepFilterNet 100%",
            &run(&mut dfn(1.0), &noisy),
            deepfilter::DELAY,
        );
        if std::env::var_os("ALL").is_some() {
            show("RNNoise 100%", &run(&mut rnn(1.0), &noisy), FRAME);
            show(
                "DeepFilterNet 80%",
                &run(&mut dfn(0.8), &noisy),
                deepfilter::DELAY,
            );
            {
                let a = run(&mut dfn(1.0), &noisy);
                show(
                    "DeepFilterNet 100% -> RNNoise 100%",
                    &run(&mut rnn(1.0), &a),
                    deepfilter::DELAY + FRAME,
                );
            }
            for thr in [-30.0f32, -26.0] {
                let chain = Arc::new(ChainShared::new(Levels {
                    gate_enabled: true,
                    gate_threshold_db: thr,
                    ..Default::default()
                }));
                let mut p = ProcessingChain::new(Box::new(dfn(1.0)), chain);
                show(
                    &format!("DeepFilterNet 100% + gate {thr:.0} dB"),
                    &run(&mut p, &noisy),
                    deepfilter::DELAY,
                );
            }
            show("hush-vani (16 kHz)", &hush(&noisy, &hush_model), 0);
        }
        // Speech-cut check: share of voice-active 20 ms blocks where isolation is more than 6 dB
        // quieter than DeepFilterNet alone (i.e. it clipped the user's own speech).
        let base = run(&mut dfn(1.0), &noisy);
        let cut_pct = |out: &[f32]| -> f64 {
            let d = deepfilter::DELAY;
            let (mut n, mut cut) = (0usize, 0usize);
            for (k, on) in m.target.iter().enumerate() {
                let a = k * 960 + d;
                if !on || k < 100 || a + 960 > out.len() {
                    continue;
                }
                n += 1;
                if db(rms(&out[a..a + 960])) < db(rms(&base[a..a + 960])) - 6.0 {
                    cut += 1;
                }
            }
            100.0 * cut as f64 / n.max(1) as f64
        };
        for margin in [3.0f32, 4.5, 6.0, 9.0] {
            let chain = Arc::new(ChainShared::new(Levels {
                isolation_enabled: true,
                isolation_margin_db: margin,
                ..Default::default()
            }));
            let mut p = ProcessingChain::new(Box::new(dfn(1.0)), chain);
            let out = run(&mut p, &noisy);
            show(
                &format!("DeepFilterNet 100% + isolation {margin:.1} dB"),
                &out,
                deepfilter::DELAY,
            );
            println!(
                "{:>38}  voice blocks cut by >6 dB: {:.1}%",
                "",
                cut_pct(&out)
            );
        }
        println!();
    }
}

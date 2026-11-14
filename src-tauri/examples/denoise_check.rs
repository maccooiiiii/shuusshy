//! Hardware validation for Milestones 4-5 (RNNoise vs DeepFilterNet): measures what RNNoise actually does to the user's real
//! microphone audio, end to end through the virtual cable.
//!
//! Runs the real pipeline with the RNNoise engine and records the microphone directly and
//! "CABLE Output" at the same time, in two phases:
//! 1. **Bypass** (suppression off): CABLE Output must equal the delayed microphone signal
//!    (correlation ≈ 1, level change ≈ 0 dB). This also measures the alignment lag.
//! 2. **RNNoise on** (strength 100 %): using that alignment, compares 20 ms blocks of mic vs
//!    cable: the quietest blocks (room noise) and the loudest blocks (speech).
//!
//! Close the Shuusshy app first. Talk with pauses during both phases.
//!
//! ```text
//! cargo run --release --example denoise_check -- [seconds_per_phase=10]
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, SampleFormat};
use shuusshy_lib::audio::devices::{
    enumerate_once,
    model::{CableStatus, auto_select, cable_status},
};
use shuusshy_lib::audio::engines::{
    EngineShared, deepfilter::DeepFilterProcessor, rnnoise::RnnoiseProcessor,
};
use shuusshy_lib::audio::pipeline::Pipeline;

type Rec = Arc<Mutex<(Vec<f32>, Option<i128>)>>;

fn record(id: &str, cap: usize) -> (cpal::Stream, Rec) {
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();
    let dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, id))
        .unwrap();
    let cfg = dev.default_input_config().unwrap();
    assert_eq!(cfg.sample_format(), SampleFormat::F32);
    assert_eq!(cfg.sample_rate(), 48_000, "analysis assumes 48 kHz");
    let ch = cfg.channels() as usize;
    let rec: Rec = Arc::new(Mutex::new((Vec::with_capacity(cap), None)));
    let r = rec.clone();
    let stream = dev
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
            |_| {},
            None,
        )
        .unwrap();
    (stream, rec)
}

/// Records both streams for `secs`; returns (mic, cable, cable start − mic start in samples).
fn capture(mic_id: &str, cable_id: &str, secs: f64) -> (Vec<f32>, Vec<f32>, f64) {
    let cap = (48_000.0 * (secs + 1.0)) as usize;
    let (sm, rm) = record(mic_id, cap);
    let (sc, rc) = record(cable_id, cap);
    sm.play().unwrap();
    sc.play().unwrap();
    std::thread::sleep(Duration::from_secs_f64(secs));
    drop(sm);
    drop(sc);
    let (m, tm) = rm.lock().unwrap().clone();
    let (c, tc) = rc.lock().unwrap().clone();
    let offset = (tc.unwrap() - tm.unwrap()) as f64 / 1e9 * 48_000.0;
    (m, c, offset)
}

/// Per-20 ms-block voice probability of the raw mic signal, from a separate RNNoise instance
/// used only as a voice detector (max of its two 10 ms frames).
fn voice_probability(mic: &[f32]) -> Vec<f32> {
    let mut st = nnnoiseless::DenoiseState::new();
    let (mut out, mut inp) = ([0.0f32; 480], [0.0f32; 480]);
    let mut frames = Vec::new();
    for f in mic.as_chunks::<480>().0 {
        for (o, &x) in inp.iter_mut().zip(f) {
            *o = x * 32768.0;
        }
        frames.push(st.process_frame(&mut out, &inp));
    }
    frames
        .chunks(2)
        .map(|c| c.iter().cloned().fold(0.0, f32::max))
        .collect()
}

fn ncc(a: &[f32], b: &[f32], k: isize) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &x) in a.iter().enumerate() {
        let j = i as isize + k;
        if j < 0 || j as usize >= b.len() {
            continue;
        }
        let (x, y) = (x as f64, b[j as usize] as f64);
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa * bb).sqrt().max(1e-30)
}

/// Best lag of `b` relative to `a` within ±400 ms (coarse on 4× decimated, then fine).
fn best_lag(a: &[f32], b: &[f32]) -> (isize, f64) {
    let dec = |x: &[f32]| {
        x.as_chunks::<4>()
            .0
            .iter()
            .map(|c| c.iter().sum::<f32>() / 4.0)
            .collect::<Vec<_>>()
    };
    let (ad, bd) = (dec(a), dec(b));
    let (mut bk, mut best) = (0isize, f64::MIN);
    for k in -4800isize..=4800 {
        let v = ncc(&ad, &bd, k);
        if v > best {
            (best, bk) = (v, k);
        }
    }
    let (mut fk, mut fine) = (bk * 4, f64::MIN);
    for k in bk * 4 - 8..=bk * 4 + 8 {
        let v = ncc(a, b, k);
        if v > fine {
            (fine, fk) = (v, k);
        }
    }
    (fk, fine)
}

/// 20 ms block levels (dBFS) of `a` and of `b` shifted by `lag`, over the common range.
fn block_levels(a: &[f32], b: &[f32], lag: isize) -> Vec<(f64, f64)> {
    let block = 960;
    let db = |x: &[f32]| {
        let ms = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64;
        10.0 * ms.max(1e-20).log10()
    };
    let mut out = Vec::new();
    let mut i = 24_000usize; // skip first 0.5 s
    while i + block < a.len() {
        let j = i as isize + lag;
        if j < 0 || (j as usize) + block >= b.len() {
            break;
        }
        let j = j as usize;
        out.push((db(&a[i..i + block]), db(&b[j..j + block])));
        i += block;
    }
    out
}

fn percentile(mut v: Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p) as usize]
}

struct Measured {
    noise_removed: Option<f64>,
    speech_change: Option<f64>,
    speech_p25: Option<f64>,
    blocks: (usize, usize),
    avg_ms: f32,
    bad_frames: u64,
    underruns: u64,
}

/// Runs one engine: a bypass phase (measures latency) then a full-strength phase (measures
/// noise removed where no voice is detected, and the speech level where voice is detected).
fn measure(
    kind: &str,
    mic_id: &str,
    render_id: &str,
    cable_id: &str,
    secs: f64,
    phase: &mut u32,
) -> Measured {
    let engine = Arc::new(EngineShared::new(false, 1.0));
    let processor: Box<dyn shuusshy_lib::audio::pipeline::FrameProcessor> = match kind {
        "RNNoise" => Box::new(RnnoiseProcessor::new(engine.clone())),
        _ => Box::new(DeepFilterProcessor::new(engine.clone())),
    };
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let e2 = errors.clone();
    let pipeline = Pipeline::start(
        mic_id,
        render_id,
        processor,
        Arc::new(move |e| e2.lock().unwrap().push(e)),
    )
    .expect("pipeline start");
    std::thread::sleep(Duration::from_millis(1500));

    *phase += 1;
    println!(">>> PHASE {phase} of 4 — {kind} OFF — keep talking with pauses");
    let (m1, c1, off1) = capture(mic_id, cable_id, secs);
    let (lag, corr) = best_lag(&m1, &c1);
    let latency = lag as f64 + off1;
    let levels = block_levels(&m1, &c1, lag);
    let diff: Vec<f64> = levels.iter().map(|(a, b)| b - a).collect();
    println!(
        "    bypass: level change {:+.2} dB (median), latency {:.1} ms (r={corr:.2})",
        percentile(diff, 0.5),
        latency / 48.0
    );

    engine.set(true, 1.0);
    std::thread::sleep(Duration::from_millis(500));
    *phase += 1;
    println!(">>> PHASE {phase} of 4 — {kind} ON (100 %) — keep talking with pauses");
    let (m2, c2, off2) = capture(mic_id, cable_id, secs);
    let lag2 = (latency - off2).round() as isize;
    let levels = block_levels(&m2, &c2, lag2);
    let vad = voice_probability(&m2);
    let (mut speech, mut noise) = (Vec::new(), Vec::new());
    for (k, (a, b)) in levels.iter().enumerate() {
        let p = vad.get(25 + k).copied().unwrap_or(0.0);
        if p > 0.9 {
            speech.push(b - a);
        } else if p < 0.1 {
            noise.push(a - b);
        }
    }
    let t = engine.telemetry();
    let st = pipeline.stats.snapshot();
    assert!(!t.overloaded, "{kind} reported overload");
    Measured {
        noise_removed: (!noise.is_empty()).then(|| percentile(noise.clone(), 0.5)),
        speech_change: (!speech.is_empty()).then(|| percentile(speech.clone(), 0.5)),
        speech_p25: (!speech.is_empty()).then(|| percentile(speech.clone(), 0.25)),
        blocks: (speech.len(), noise.len()),
        avg_ms: t.avg_ms,
        bad_frames: t.bad_frames,
        underruns: st.underruns,
    }
}

fn main() {
    let secs: f64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(8.0);
    let snap = enumerate_once().expect("enumerate");
    let mic = auto_select(&snap.inputs).expect("no physical mic").clone();
    let CableStatus::Ready {
        render,
        capture: cable,
    } = cable_status(&snap.inputs, &snap.outputs)
    else {
        panic!("VB-CABLE not ready");
    };
    println!("mic: {}\n", mic.name);
    let mut phase = 0;
    let results: Vec<(&str, Measured)> = ["RNNoise", "DeepFilterNet"]
        .into_iter()
        .map(|k| {
            (
                k,
                measure(k, &mic.id, &render.id, &cable.id, secs, &mut phase),
            )
        })
        .collect();

    let fmt = |v: Option<f64>, signed: bool| match v {
        Some(x) if signed => format!("{x:+.1} dB"),
        Some(x) => format!("{x:.1} dB"),
        None => "n/a".into(),
    };
    println!(
        "\n{:<14} {:>15} {:>14} {:>12} {:>16} {:>10} {:>9}",
        "engine",
        "noise removed",
        "speech (med)",
        "speech p25",
        "blocks sp/noise",
        "ms/frame",
        "dropouts"
    );
    for (k, m) in &results {
        println!(
            "{:<14} {:>15} {:>14} {:>12} {:>16} {:>10.3} {:>9}{}",
            k,
            fmt(m.noise_removed, false),
            fmt(m.speech_change, true),
            fmt(m.speech_p25, true),
            format!("{}/{}", m.blocks.0, m.blocks.1),
            m.avg_ms,
            m.underruns,
            if m.bad_frames > 0 {
                format!("  ({} bad frames)", m.bad_frames)
            } else {
                String::new()
            }
        );
    }
}

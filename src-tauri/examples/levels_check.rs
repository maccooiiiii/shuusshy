//! Hardware validation for Milestone 6: gains and noise gate through the real pipeline.
//!
//! Runs the real processing chain (engine bypassed) from the microphone to CABLE Input and
//! compares 20 ms block levels of the microphone and CABLE Output, using the room's own
//! background noise as the signal (no talking needed). Every setting is compared with the
//! "all controls neutral" control measurement, so the path's own response at the measured
//! signal level (e.g. resampler band-limiting of a near-silent mic's quantization noise) cancels
//! out. Close the Shuusshy app first.
//!
//! ```text
//! cargo run --release --example levels_check
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, SampleFormat};
use shuusshy_lib::audio::chain::{ChainShared, Levels, ProcessingChain};
use shuusshy_lib::audio::devices::{
    enumerate_once,
    model::{CableStatus, auto_select, cable_status},
};
use shuusshy_lib::audio::engines::{EngineShared, rnnoise::RnnoiseProcessor};
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

fn main() {
    let snap = enumerate_once().expect("enumerate");
    let mic = auto_select(&snap.inputs).expect("no physical mic").clone();
    let CableStatus::Ready {
        render,
        capture: cable,
    } = cable_status(&snap.inputs, &snap.outputs)
    else {
        panic!("VB-CABLE not ready");
    };
    let chain = Arc::new(ChainShared::new(Levels::default()));
    // Engine present but switched off (exact bypass), so only gains and gate act.
    let engine = Arc::new(EngineShared::new(false, 1.0));
    let processor = Box::new(ProcessingChain::new(
        Box::new(RnnoiseProcessor::new(engine)),
        chain.clone(),
    ));
    let pipeline = Pipeline::start(
        &mic.id,
        &render.id,
        processor,
        Arc::new(|e| eprintln!("{e}")),
    )
    .expect("pipeline");
    std::thread::sleep(Duration::from_millis(1500));

    // Reference phase: measures latency and the room's noise level.
    let (m, c, off) = capture(&mic.id, &cable.id, 5.0);
    let (lag, corr) = best_lag(&m, &c);
    let latency = lag as f64 + off;
    let levels = block_levels(&m, &c, lag);
    let mic_db: Vec<f64> = levels.iter().map(|l| l.0).collect();
    let floor = percentile(mic_db.clone(), 0.5);
    let loud = percentile(mic_db, 0.95);
    println!(
        "mic: {}  (room level median {floor:.1} dBFS, p95 {loud:.1} dBFS; r={corr:.2})\n",
        mic.name
    );
    println!(
        "{:<44} {:>10} {:>10}",
        "setting (vs neutral control)", "expected", "measured"
    );

    let cases: Vec<(&str, Levels, Option<f64>)> = vec![
        ("all controls neutral", Levels::default(), Some(0.0)),
        (
            "input gain +6 dB",
            Levels {
                input_gain_db: 6.0,
                ..Default::default()
            },
            Some(6.0),
        ),
        (
            "output gain -10 dB",
            Levels {
                output_gain_db: -10.0,
                ..Default::default()
            },
            Some(-10.0),
        ),
        (
            "input +12, output -6 dB",
            Levels {
                input_gain_db: 12.0,
                output_gain_db: -6.0,
                ..Default::default()
            },
            Some(6.0),
        ),
        (
            "gate on, threshold 10 dB above room level",
            Levels {
                gate_enabled: true,
                gate_threshold_db: (loud + 10.0) as f32,
                ..Default::default()
            },
            None,
        ),
        (
            "gate on, threshold -90 dB (below room)",
            Levels {
                gate_enabled: true,
                gate_threshold_db: -90.0,
                ..Default::default()
            },
            Some(0.0),
        ),
    ];
    let mut failures = 0;
    let mut control: Option<f64> = None;
    for (name, l, expect) in cases {
        chain.set(l);
        std::thread::sleep(Duration::from_millis(800)); // let the gate hold/release settle
        let (m, c, off) = capture(&mic.id, &cable.id, 4.0);
        let lag = (latency - off).round() as isize;
        let diff: Vec<f64> = block_levels(&m, &c, lag)
            .iter()
            .map(|(a, b)| b - a)
            .collect();
        let raw = percentile(diff, 0.5);
        let reference = *control.get_or_insert(raw);
        let measured = raw - reference;
        let ok = match expect {
            Some(e) => (measured - e).abs() < 0.5,
            None => measured < -40.0,
        };
        if !ok {
            failures += 1;
        }
        println!(
            "{:<44} {:>10} {:>+9.1} dB {}   (raw {:+.1} dB)",
            name,
            expect.map_or("< -40 dB".to_string(), |e| format!("{e:+.1} dB")),
            measured,
            if ok { "OK" } else { "FAIL" },
            raw
        );
    }
    let t = chain.telemetry();
    println!(
        "\nlimiter frames: {}, dropouts: {}, gate open at end: {}",
        t.limited_frames,
        pipeline.stats.snapshot().underruns,
        t.gate_open
    );
    println!(
        "{}",
        if failures == 0 {
            "LEVELS VERIFIED"
        } else {
            "LEVELS NOT VERIFIED"
        }
    );
}

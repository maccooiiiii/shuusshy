//! Hardware verification for Milestone 3: proves that the microphone's audio really arrives on the
//! virtual microphone ("CABLE Output") and measures end-to-end latency and stability.
//!
//! It starts the real Shuusshy pipeline (selected mic -> CABLE Input), then records the
//! microphone directly AND CABLE Output (what Teams/Zoom would hear) at the same time, and
//! cross-correlates the two using each stream's capture timestamps.
//!
//! Make sure the Shuusshy app is NOT running (two writers to CABLE Input would mix).
//!
//! ```text
//! cargo run --release --example route_check -- [record_seconds=6] [soak_seconds=0]
//! ```

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, Sample, SampleFormat};
use shuusshy_lib::audio::devices::{
    enumerate_once,
    model::{CableStatus, auto_select, cable_status},
};
use shuusshy_lib::audio::engines::{EngineShared, rnnoise::RnnoiseProcessor};
use shuusshy_lib::audio::pipeline::{Passthrough, Pipeline};

struct Recording {
    samples: Vec<f32>,
    first_capture_ns: Option<i128>,
    rate: u32,
}

fn record(endpoint_id: &str, label: &str, seconds: f64) -> (cpal::Stream, Arc<Mutex<Recording>>) {
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();
    let dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, endpoint_id))
        .unwrap_or_else(|| panic!("{label}: device not found"));
    let cfg = dev.default_input_config().unwrap();
    assert_eq!(
        cfg.sample_format(),
        SampleFormat::F32,
        "{label}: expected f32 shared-mode format"
    );
    let ch = cfg.channels() as usize;
    let rate = cfg.sample_rate();
    let rec = Arc::new(Mutex::new(Recording {
        samples: Vec::with_capacity((rate as f64 * (seconds + 1.0)) as usize),
        first_capture_ns: None,
        rate,
    }));
    let r = rec.clone();
    let stream = dev
        .build_input_stream(
            cfg.config(),
            move |data: &[f32], info| {
                let mut r = r.lock().unwrap();
                if r.first_capture_ns.is_none() {
                    r.first_capture_ns = Some(info.timestamp().capture.as_nanos() as i128);
                }
                let cap = r.samples.capacity();
                for f in data.chunks_exact(ch) {
                    if r.samples.len() < cap {
                        r.samples
                            .push(f.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / ch as f32);
                    }
                }
            },
            move |e| eprintln!("recorder error: {e}"),
            None,
        )
        .unwrap();
    (stream, rec)
}

fn rms_db(x: &[f32]) -> f64 {
    let ms = x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * ms.max(1e-20).log10()
}

/// Normalised cross-correlation of `a[i]` with `b[i + k]` over `a[start..start+len]`.
fn ncc(a: &[f32], b: &[f32], start: usize, len: usize, k: isize) -> f64 {
    let (mut sab, mut saa, mut sbb) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &x) in a.iter().enumerate().skip(start).take(len) {
        let j = i as isize + k;
        if j < 0 || j as usize >= b.len() {
            continue;
        }
        let (x, y) = (x as f64, b[j as usize] as f64);
        sab += x * y;
        saa += x * x;
        sbb += y * y;
    }
    sab / (saa * sbb).sqrt().max(1e-30)
}

fn decimate(x: &[f32], f: usize) -> Vec<f32> {
    x.chunks_exact(f)
        .map(|c| c.iter().sum::<f32>() / f as f32)
        .collect()
}

fn main() {
    let args: Vec<f64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let rec_secs = args.first().copied().unwrap_or(6.0);
    let soak_secs = args.get(1).copied().unwrap_or(0.0);

    let snap = enumerate_once().expect("enumerate devices");
    let mic = auto_select(&snap.inputs)
        .expect("no physical microphone")
        .clone();
    let CableStatus::Ready { render, capture } = cable_status(&snap.inputs, &snap.outputs) else {
        panic!("VB-CABLE not ready");
    };
    println!("mic:          {}", mic.name);
    println!("pipeline out: {}", render.name);
    println!("verify from:  {}", capture.name);

    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let e2 = errors.clone();
    let pipeline = Pipeline::start(
        &mic.id,
        &render.id,
        // SHUUSSHY_RNNOISE=1 runs the soak with RNNoise enabled at full strength (the routing
        // correlation proof is then expected to fail, since the audio is deliberately changed).
        if std::env::var_os("SHUUSSHY_RNNOISE").is_some() {
            println!("engine: RNNoise enabled (100 %)");
            Box::new(RnnoiseProcessor::new(Arc::new(EngineShared::new(
                true, 1.0,
            )))) as Box<dyn shuusshy_lib::audio::pipeline::FrameProcessor>
        } else {
            Box::new(Passthrough)
        },
        Arc::new(move |e| e2.lock().unwrap().push(e)),
    )
    .expect("pipeline start");
    println!("pipeline: {:?}", pipeline.info);

    // Let buffers prime and the drift loop settle a little.
    std::thread::sleep(Duration::from_millis(1500));

    let (s_mic, r_mic) = record(&mic.id, "mic", rec_secs);
    let (s_cab, r_cab) = record(&capture.id, "cable", rec_secs);
    s_mic.play().unwrap();
    s_cab.play().unwrap();
    std::thread::sleep(Duration::from_secs_f64(rec_secs));
    drop(s_mic);
    drop(s_cab);

    let m = r_mic.lock().unwrap();
    let c = r_cab.lock().unwrap();
    assert_eq!(m.rate, c.rate, "analysis assumes equal rates");
    let rate = m.rate as f64;
    println!(
        "recorded: mic {} samples ({:.1} dBFS), cable {} samples ({:.1} dBFS)",
        m.samples.len(),
        rms_db(&m.samples),
        c.samples.len(),
        rms_db(&c.samples)
    );

    // Offset between the two recordings' first captured samples.
    let d_s = (c.first_capture_ns.unwrap() - m.first_capture_ns.unwrap()) as f64 / 1e9;
    println!(
        "recording start offset (cable - mic): {:.2} ms",
        d_s * 1000.0
    );

    // Coarse search on 4x decimated signals over +/-500 ms, then refine at full rate.
    let f = 4;
    let (md, cd) = (decimate(&m.samples, f), decimate(&c.samples, f));
    let start_d = (0.5 * rate) as usize / f;
    let len_d = ((rec_secs - 1.5) * rate) as usize / f;
    let max_k = (0.5 * rate) as isize / f as isize;
    let (mut best_k, mut best) = (0isize, f64::MIN);
    for k in -max_k..=max_k {
        let v = ncc(&md, &cd, start_d, len_d, k);
        if v > best {
            best = v;
            best_k = k;
        }
    }
    let (start, len) = (start_d * f, len_d * f);
    let (mut fine_k, mut fine) = (best_k * f as isize, f64::MIN);
    for k in best_k * f as isize - 2 * f as isize..=best_k * f as isize + 2 * f as isize {
        let v = ncc(&m.samples, &c.samples, start, len, k);
        if v > fine {
            fine = v;
            fine_k = k;
        }
    }
    let latency_ms = (fine_k as f64 / rate + d_s) * 1000.0;
    println!("correlation peak: {fine:.4} at lag {fine_k} samples");
    println!("END-TO-END LATENCY (mic capture -> CABLE Output capture): {latency_ms:.1} ms");

    // Control: correlation of the mic against a far-away lag should be low.
    let control = ncc(
        &m.samples,
        &c.samples,
        start,
        len,
        fine_k + (0.2 * rate) as isize,
    );
    println!("control correlation at +200 ms: {control:.4}");

    // Gap / click analysis (skips the first 0.5 s). A real microphone never produces runs of
    // exact digital zeros, but Shuusshy pads underruns with them, so zero runs = dropouts.
    // Clicks: sample-to-sample jumps far larger than the local signal's own movement. The
    // direct mic recording is the baseline; extra events in CABLE Output were added on the way.
    let skip = (0.5 * rate) as usize;
    let gaps = |x: &[f32]| {
        let (mut runs, mut longest, mut cur) = (0usize, 0usize, 0usize);
        for &v in &x[skip.min(x.len())..] {
            if v == 0.0 {
                cur += 1;
            } else {
                if cur >= 48 {
                    runs += 1;
                }
                longest = longest.max(cur);
                cur = 0;
            }
        }
        (runs, longest as f64 / rate * 1000.0)
    };
    let clicks = |x: &[f32]| {
        let block = 480;
        let mut n = 0;
        for w in x[skip.min(x.len())..].chunks(block) {
            let d: Vec<f32> = w.windows(2).map(|p| p[1] - p[0]).collect();
            let rms = (d.iter().map(|v| v * v).sum::<f32>() / d.len().max(1) as f32).sqrt();
            n += d
                .iter()
                .filter(|v| v.abs() > (10.0 * rms).max(0.02))
                .count();
        }
        n
    };
    let (mg, ml) = gaps(&m.samples);
    let (cg, cl) = gaps(&c.samples);
    println!("silent gaps >=1 ms: mic {mg} (longest {ml:.1} ms), cable {cg} (longest {cl:.1} ms)");
    println!(
        "click-like jumps: mic {}, cable {}",
        clicks(&m.samples),
        clicks(&c.samples)
    );

    let st = pipeline.stats.snapshot();
    println!("pipeline stats: {st:?}");
    let errs = errors.lock().unwrap().clone();
    println!("stream errors: {errs:?}");

    let ok = fine > 0.8 && control.abs() < 0.5 && errs.is_empty();
    println!("ROUTING {}", if ok { "VERIFIED" } else { "NOT VERIFIED" });
    drop(m);
    drop(c);

    if soak_secs > 0.0 {
        // Optional synthetic CPU load (normal-priority busy threads) from `load_at` seconds on,
        // to check that real-time audio survives a busy machine (e.g. a video call).
        let load_threads = args.get(2).copied().unwrap_or(0.0) as usize;
        let load_at = args.get(3).copied().unwrap_or(0.0);
        let stop_load = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut load_started = false;
        println!(
            "\nsoak test for {soak_secs} s (load: {load_threads} threads from t={load_at}s) ..."
        );
        pipeline.stats.take_peaks();
        let t0 = Instant::now();
        let mut prev_underruns = 0;
        while t0.elapsed().as_secs_f64() < soak_secs {
            if load_threads > 0 && !load_started && t0.elapsed().as_secs_f64() >= load_at {
                load_started = true;
                println!("--- starting {load_threads} busy threads ---");
                for _ in 0..load_threads {
                    let stop = stop_load.clone();
                    std::thread::spawn(move || {
                        let mut x = 1u64;
                        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                            std::hint::black_box(x);
                        }
                    });
                }
            }
            std::thread::sleep(Duration::from_secs(10));
            let s = pipeline.stats.snapshot();
            let p = pipeline.stats.take_peaks();
            println!(
                "t={:>4.0}s buf={:>5.1}ms drift={:>7.1}ppm under={:>2}(+{}) over={} resync={} xrun={} | out req<={} minfill={} gap<={:.1}ms | in pkt<={} gap<={:.1}ms | dsp gap<={:.1}ms frame<={:.2}ms | err={}",
                t0.elapsed().as_secs_f64(),
                s.buffered_ms,
                s.drift_ppm,
                s.underruns,
                s.underruns - prev_underruns,
                s.overruns,
                s.resyncs,
                s.xruns,
                p.out_max_request,
                p.out_min_fill,
                p.out_max_gap_ms,
                p.in_max_packet,
                p.in_max_gap_ms,
                p.dsp_max_gap_ms,
                s.max_frame_ms,
                errors.lock().unwrap().len()
            );
            prev_underruns = s.underruns;
        }
        stop_load.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    drop(pipeline);
    if !ok {
        std::process::exit(1);
    }
}

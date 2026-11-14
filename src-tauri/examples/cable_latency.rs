//! Measures VB-CABLE's own latency (CABLE Input playback -> CABLE Output capture), independent
//! of Shuusshy's pipeline, by playing deterministic noise into CABLE Input and correlating it with
//! what arrives on CABLE Output, using the streams' playback/capture timestamps.
//!
//! ```text
//! cargo run --release --example cable_latency
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId};
use shuusshy_lib::audio::devices::{
    enumerate_once,
    model::{CableStatus, cable_status},
};

fn lcg(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*state >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.2
}

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

fn main() {
    let snap = enumerate_once().unwrap();
    let CableStatus::Ready { render, capture } = cable_status(&snap.inputs, &snap.outputs) else {
        panic!("VB-CABLE not ready");
    };
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();
    let out_dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, &render.id))
        .unwrap();
    let in_dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, &capture.id))
        .unwrap();
    let ocfg = out_dev.default_output_config().unwrap();
    let icfg = in_dev.default_input_config().unwrap();
    assert_eq!(ocfg.sample_rate(), icfg.sample_rate());
    let rate = ocfg.sample_rate() as f64;
    let (och, ich) = (ocfg.channels() as usize, icfg.channels() as usize);

    let played = Arc::new(Mutex::new((
        Vec::<f32>::with_capacity(48_000 * 8),
        None::<i128>,
    )));
    let p = played.clone();
    let mut seed = 12345u32;
    let out = out_dev
        .build_output_stream(
            ocfg.config(),
            move |data: &mut [f32], info| {
                let mut p = p.lock().unwrap();
                if p.1.is_none() {
                    p.1 = Some(info.timestamp().playback.as_nanos() as i128);
                }
                for f in data.chunks_exact_mut(och) {
                    let v = lcg(&mut seed);
                    f.fill(v);
                    if p.0.len() < p.0.capacity() {
                        p.0.push(v);
                    }
                }
            },
            |e| eprintln!("out err: {e}"),
            None,
        )
        .unwrap();
    let got = Arc::new(Mutex::new((
        Vec::<f32>::with_capacity(48_000 * 8),
        None::<i128>,
    )));
    let g = got.clone();
    let inp = in_dev
        .build_input_stream(
            icfg.config(),
            move |data: &[f32], info| {
                let mut g = g.lock().unwrap();
                if g.1.is_none() {
                    g.1 = Some(info.timestamp().capture.as_nanos() as i128);
                }
                for f in data.chunks_exact(ich) {
                    if g.0.len() < g.0.capacity() {
                        g.0.push(f[0]);
                    }
                }
            },
            |e| eprintln!("in err: {e}"),
            None,
        )
        .unwrap();
    out.play().unwrap();
    inp.play().unwrap();
    std::thread::sleep(Duration::from_secs(5));
    drop(inp);
    drop(out);

    let (pl, pt) = played.lock().unwrap().clone();
    let (gt_s, gt) = got.lock().unwrap().clone();
    let d = (gt.unwrap() - pt.unwrap()) as f64 / 1e9;
    let start = (1.0 * rate) as usize;
    let len = (2.0 * rate) as usize;
    let (mut bk, mut best) = (0isize, f64::MIN);
    for k in (-(0.5 * rate) as isize)..((0.5 * rate) as isize) {
        let v = ncc(&pl, &gt_s, start, len, k);
        if v > best {
            best = v;
            bk = k;
        }
    }
    println!("cable format: {} Hz, {} ch out / {} ch in", rate, och, ich);
    println!(
        "correlation {best:.4} at lag {bk}; start offset {:.2} ms",
        d * 1000.0
    );
    println!(
        "VB-CABLE LATENCY (CABLE Input playback timestamp -> CABLE Output capture timestamp): {:.1} ms",
        (bk as f64 / rate + d) * 1000.0
    );
}

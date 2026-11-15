//! QA tool (Milestone 11): runs a recorded voice (mono 48 kHz WAV, e.g. from
//! tools/qa-record) through Shuusshy's real processing chain in several configurations and
//! writes one WAV per configuration, so they can be compared by ear and by numbers.
//!
//! ```text
//! cargo run --release --example voice_variants -- voice.wav out_dir
//! ```

use std::sync::Arc;

use shuusshy_lib::audio::chain::{ChainShared, Levels, ProcessingChain};
use shuusshy_lib::audio::engines::{
    EngineShared, deepfilter::DeepFilterProcessor, rnnoise::RnnoiseProcessor,
};
use shuusshy_lib::audio::pipeline::{FRAME, FrameProcessor};

/// Reads a mono WAV with 32-bit float or 16-bit integer samples.
fn read_wav(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).expect("read wav");
    let (mut i, mut fmt, mut bits) = (12usize, 3u16, 32u16);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " {
            fmt = u16::from_le_bytes([body[0], body[1]]);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            return match (fmt, bits) {
                (3 | 0xFFFE, 32) => body
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes(*c))
                    .collect(),
                (1, 16) => body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0)
                    .collect(),
                _ => panic!("unsupported WAV format {fmt}/{bits}"),
            };
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

fn write_wav(path: &std::path::Path, x: &[f32]) {
    let data_len = (x.len() * 4) as u32;
    let mut b = Vec::with_capacity(44 + x.len() * 4);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for v in x {
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).expect("write wav");
}

fn db(v: f64) -> f64 {
    20.0 * v.max(1e-10).log10()
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

struct Variant {
    name: &'static str,
    file: &'static str,
    dfn: bool,
    strength: f32,
    input_gain_db: f32,
    isolation: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = read_wav(args.get(1).expect("usage: voice.wav out_dir"));
    let out = std::path::PathBuf::from(args.get(2).expect("usage: voice.wav out_dir"));
    std::fs::create_dir_all(&out).unwrap();
    let variants = [
        Variant {
            name: "A current settings: +10.5 dB in, RNNoise 100%, isolation 6 dB",
            file: "A-current-settings.wav",
            dfn: false,
            strength: 1.0,
            input_gain_db: 10.5,
            isolation: true,
        },
        Variant {
            name: "B input gain 0 dB, RNNoise 100%, isolation 6 dB",
            file: "B-gain0-rnnoise-isolation.wav",
            dfn: false,
            strength: 1.0,
            input_gain_db: 0.0,
            isolation: true,
        },
        Variant {
            name: "C input gain 0 dB, RNNoise 100%, no isolation",
            file: "C-gain0-rnnoise.wav",
            dfn: false,
            strength: 1.0,
            input_gain_db: 0.0,
            isolation: false,
        },
        Variant {
            name: "D input gain 0 dB, RNNoise 80%, no isolation",
            file: "D-gain0-rnnoise80.wav",
            dfn: false,
            strength: 0.8,
            input_gain_db: 0.0,
            isolation: false,
        },
        Variant {
            name: "E input gain 0 dB, DeepFilterNet 100%, no isolation",
            file: "E-gain0-deepfilternet.wav",
            dfn: true,
            strength: 1.0,
            input_gain_db: 0.0,
            isolation: false,
        },
        Variant {
            name: "F input gain 0 dB, DeepFilterNet 80%, isolation 6 dB",
            file: "F-gain0-deepfilternet80-isolation.wav",
            dfn: true,
            strength: 0.8,
            input_gain_db: 0.0,
            isolation: true,
        },
    ];
    // Voice-active 20 ms blocks of the input (within 30 dB of its loudest block).
    let blocks: Vec<f64> = input.chunks(960).map(rms).collect();
    let peak_block = blocks.iter().cloned().fold(0.0, f64::max);
    let active: Vec<bool> = blocks
        .iter()
        .map(|&b| db(b) > db(peak_block) - 30.0)
        .collect();
    println!(
        "input: {:.1} s, peak {:.1} dBFS, rms {:.1} dBFS",
        input.len() as f64 / 48_000.0,
        db(input.iter().fold(0f32, |m, v| m.max(v.abs())) as f64),
        db(rms(&input))
    );
    println!(
        "{:<62} {:>8} {:>8} {:>9} {:>10}",
        "configuration", "peak", "rms", "limited", "voice cut"
    );
    for v in &variants {
        let eng_shared = Arc::new(EngineShared::new(true, v.strength));
        let engine: Box<dyn FrameProcessor> = if v.dfn {
            Box::new(DeepFilterProcessor::with_watchdog(eng_shared, None))
        } else {
            Box::new(RnnoiseProcessor::new(eng_shared))
        };
        let delay = engine.delay_frames() * FRAME;
        let shared = Arc::new(ChainShared::new(Levels {
            input_gain_db: v.input_gain_db,
            isolation_enabled: v.isolation,
            isolation_margin_db: 6.0,
            ..Default::default()
        }));
        let mut chain = ProcessingChain::new(engine, shared.clone());
        let mut y = Vec::with_capacity(input.len());
        for c in input.as_chunks::<FRAME>().0 {
            let mut f = *c;
            chain.process(&mut f);
            y.extend_from_slice(&f);
        }
        // Align for the comparison (remove the engine delay).
        let y: Vec<f32> = y.into_iter().skip(delay).collect();
        let gain = 10f64.powf(v.input_gain_db as f64 / 20.0);
        let cut = y
            .chunks(960)
            .zip(input.chunks(960))
            .zip(&active)
            .filter(|((_, _), a)| **a)
            .filter(|((o, i), _)| db(rms(o)) < db(rms(i) * gain) - 6.0)
            .count();
        let n_active = active.iter().filter(|a| **a).count().max(1);
        // Loudest quarter of the input blocks: certainly speech. How much do they change?
        let mut order: Vec<usize> = (0..blocks.len().min(y.len() / 960)).collect();
        order.sort_by(|a, b| blocks[*b].partial_cmp(&blocks[*a]).unwrap());
        let top = &order[..(order.len() / 4).max(1)];
        let loud_change = top
            .iter()
            .map(|&k| db(rms(&y[k * 960..(k + 1) * 960])) - db(blocks[k] * gain))
            .sum::<f64>()
            / top.len() as f64;
        let quiet: Vec<usize> = order[order.len() * 3 / 4..].to_vec();
        let quiet_in =
            quiet.iter().map(|&k| db(blocks[k])).sum::<f64>() / quiet.len().max(1) as f64;
        let quiet_out = quiet
            .iter()
            .map(|&k| db(rms(&y[k * 960..(k + 1) * 960])))
            .sum::<f64>()
            / quiet.len().max(1) as f64;
        println!(
            "{:>62} loudest speech {:+.1} dB vs input; quietest blocks {:.1} -> {:.1} dBFS",
            "", loud_change, quiet_in, quiet_out
        );
        println!(
            "{:<62} {:>6.1}dB {:>6.1}dB {:>9} {:>9.1}%",
            v.name,
            db(y.iter().fold(0f32, |m, s| m.max(s.abs())) as f64),
            db(rms(&y)),
            shared.telemetry().limited_frames,
            100.0 * cut as f64 / n_active as f64
        );
        write_wav(&out.join(v.file), &y);
    }
}

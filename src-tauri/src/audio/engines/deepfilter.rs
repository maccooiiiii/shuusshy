//! DeepFilterNet3 engine (via `dfn3_rs`, MIT OR Apache-2.0; code and model weights).
//!
//! - Format: 48 kHz mono, 480-sample frames, normalized f32 (no scaling needed).
//! - Algorithmic delay: **1440 samples (30 ms)**: 10 ms STFT overlap plus 2 lookahead frames
//!   (verified by `delay_is_30_ms`). Kept when off, so switching never shifts the audio in time.
//! - **Strength** maps to DeepFilterNet's own *attenuation limit* (the `atten-lim-db` control):
//!   `0 %` → 0 dB (the exact delayed original), rising linearly to 45 dB at 99 %, and `100 %` →
//!   no limit. This is a genuine model parameter, not a mix.
//! - **Bypass** (suppression off) sets the limit to 0 dB, which `dfn3_rs` guarantees to be the
//!   exact delayed input (verified bit-exact by `bypass_is_the_exact_delayed_input`); a stable
//!   0 dB also skips the neural network, so bypass costs almost nothing.
//! - Every change is smoothed over 50 ms by the library, so it cannot click.
//! - Processing allocates nothing and takes no locks (library guarantee; verified with a counting
//!   allocator during the Milestone 5 spike).
//! - **Overload watchdog**: if processing takes more than 7 ms of each 10 ms frame for 20 frames
//!   in a row, the engine stops processing, passes audio through untouched, and reports the
//!   overload, rather than causing dropouts on slow CPUs.

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

use dfn3_rs::Denoiser;

use super::EngineShared;
use crate::audio::pipeline::{FRAME, FrameProcessor};

/// DeepFilterNet's algorithmic delay in samples.
pub const DELAY: usize = dfn3_rs::ALGORITHM_DELAY;
/// Attenuation limit at 99 % strength; 100 % removes the limit.
const MAX_LIMITED_DB: f32 = 45.0;
const UNLIMITED_DB: f32 = 100.0;
const RAMP: Duration = Duration::from_millis(50);

/// Strength 0..1 → DeepFilterNet attenuation limit in dB.
pub fn strength_to_db(strength: f32) -> f32 {
    if strength >= 1.0 {
        UNLIMITED_DB
    } else {
        strength.clamp(0.0, 1.0) * MAX_LIMITED_DB
    }
}

/// Detects an engine that cannot keep up in real time.
#[derive(Debug, Clone)]
pub struct Watchdog {
    limit_us: f32,
    needed: u32,
    consecutive: u32,
}

impl Watchdog {
    /// Trips after `needed` consecutive frames slower than `limit_us`.
    pub fn new(limit_us: f32, needed: u32) -> Self {
        Self {
            limit_us,
            needed,
            consecutive: 0,
        }
    }

    /// Feeds one frame's processing time; returns true once tripped.
    pub fn update(&mut self, micros: f32) -> bool {
        if micros > self.limit_us {
            self.consecutive += 1;
        } else {
            self.consecutive = 0;
        }
        self.consecutive >= self.needed
    }
}

pub struct DeepFilterProcessor {
    denoiser: Denoiser,
    shared: Arc<EngineShared>,
    applied_db: f32,
    watchdog: Option<Watchdog>,
}

impl DeepFilterProcessor {
    /// Creates the engine (allocates; call off the audio thread). The embedded weights are
    /// referenced in place, so construction is fast.
    pub fn new(shared: Arc<EngineShared>) -> Self {
        Self::with_watchdog(shared, Some(Watchdog::new(7_000.0, 20)))
    }

    /// As [`new`](Self::new) with a custom (or no) overload watchdog.
    pub fn with_watchdog(shared: Arc<EngineShared>, watchdog: Option<Watchdog>) -> Self {
        let mut denoiser = Denoiser::new();
        let db = Self::target_db(&shared);
        denoiser.set_attenuation_limit(db);
        Self {
            denoiser,
            shared,
            applied_db: db,
            watchdog,
        }
    }

    fn target_db(shared: &EngineShared) -> f32 {
        if shared.enabled() && !shared.overloaded() {
            strength_to_db(shared.strength())
        } else {
            0.0
        }
    }
}

impl FrameProcessor for DeepFilterProcessor {
    fn delay_frames(&self) -> usize {
        DELAY / FRAME
    }

    fn process(&mut self, frame: &mut [f32; FRAME]) {
        let started = Instant::now();

        let db = Self::target_db(&self.shared);
        if db != self.applied_db {
            self.denoiser.set_attenuation_limit_smooth(db, RAMP);
            self.applied_db = db;
        }

        let snr = self.denoiser.process_frame(frame);

        if frame.iter().all(|v| v.is_finite()) {
            for v in frame.iter_mut() {
                *v = v.clamp(-1.0, 1.0);
            }
        } else {
            // Never let a broken frame reach the virtual microphone; restart the model state.
            self.shared.bad_frames.fetch_add(1, Relaxed);
            frame.fill(0.0);
            self.denoiser.reset();
        }

        let us = started.elapsed().as_secs_f32() * 1e6;
        if self.watchdog.as_mut().is_some_and(|w| w.update(us)) && !self.shared.overloaded() {
            self.shared.overloaded.store(true, Relaxed);
        }
        self.shared.snr_bits.store(snr.to_bits(), Relaxed);
        self.shared.record_frame(us);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: u64) -> impl FnMut() -> f32 {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        move || {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        }
    }

    /// Speech-like voiced bursts (0.6 s on / 0.4 s off) over low-passed "fan" noise.
    fn speech_and_fan(frames: usize, seed: u64) -> (Vec<f32>, Vec<bool>) {
        let mut r = rng(seed);
        let (mut a, mut b) = (0.0f32, 0.0f32);
        let mut x = Vec::with_capacity(frames * FRAME);
        let mut voiced = Vec::with_capacity(frames * FRAME);
        for i in 0..frames * FRAME {
            let t = i as f32 / 48_000.0;
            a += 0.05 * (r() - a);
            b += 0.05 * (a - b);
            let on = (t % 1.0) < 0.6;
            let f0 = 150.0 + 30.0 * (t * 2.0).sin();
            let mut v = 0.0;
            for h in 1..25 {
                let f = f0 * h as f32;
                let w = (-((f - 700.0) / 300.0).powi(2)).exp()
                    + 0.6 * (-((f - 1200.0) / 400.0).powi(2)).exp();
                v += w * (2.0 * std::f32::consts::PI * f * t).sin();
            }
            x.push(if on { 0.08 * v } else { 0.0 } + 0.6 * b);
            voiced.push(on);
        }
        (x, voiced)
    }

    /// The watchdog is left out of the quality tests so a busy test machine cannot trip it
    /// (it is tested on its own and in `runs_in_real_time_in_development_builds`).
    fn run(enabled: bool, strength: f32, input: &[f32]) -> (Vec<f32>, Arc<EngineShared>) {
        let shared = Arc::new(EngineShared::new(enabled, strength));
        let mut p = DeepFilterProcessor::with_watchdog(shared.clone(), None);
        let mut out = Vec::with_capacity(input.len());
        for chunk in input.as_chunks::<FRAME>().0 {
            let mut f = *chunk;
            p.process(&mut f);
            out.extend_from_slice(&f);
        }
        (out, shared)
    }

    fn rms_db(x: &[f32]) -> f32 {
        let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32;
        10.0 * ms.max(1e-20).log10()
    }

    /// Level of `x` (output, delayed by DELAY) in the pauses of the input, after warm-up.
    fn pause_db(out: &[f32], voiced: &[bool]) -> f32 {
        let v: Vec<f32> = (FRAME * 100..out.len())
            .filter(|&i| i >= DELAY && !voiced[i - DELAY])
            .map(|i| out[i])
            .collect();
        rms_db(&v)
    }

    #[test]
    fn bypass_is_the_exact_delayed_input() {
        let mut r = rng(1);
        let input: Vec<f32> = (0..FRAME * 40).map(|_| r() * 0.3).collect();
        let (out, _) = run(false, 0.8, &input);
        assert!(out[..DELAY].iter().all(|&v| v == 0.0));
        assert_eq!(&out[DELAY..], &input[..input.len() - DELAY]);
    }

    #[test]
    fn strength_zero_is_also_untouched() {
        let mut r = rng(2);
        let input: Vec<f32> = (0..FRAME * 30).map(|_| r() * 0.3).collect();
        let (out, _) = run(true, 0.0, &input);
        assert_eq!(&out[DELAY..], &input[..input.len() - DELAY]);
    }

    #[test]
    fn delay_is_30_ms() {
        assert_eq!(DELAY, 1440);
        let (input, _) = speech_and_fan(300, 3);
        let (out, _) = run(true, 1.0, &input);
        let skip = FRAME * 50;
        let corr = |lag: usize| -> f64 {
            (skip..input.len() - lag)
                .map(|i| out[i + lag] as f64 * input[i] as f64)
                .sum::<f64>()
        };
        let best = (0..3000)
            .max_by(|&a, &b| corr(a).partial_cmp(&corr(b)).unwrap())
            .unwrap();
        assert!(
            (best as i64 - DELAY as i64).abs() <= 2,
            "measured delay {best}"
        );
    }

    #[test]
    fn removes_fan_noise_and_keeps_speech() {
        let (input, voiced) = speech_and_fan(1000, 4);
        let (out, shared) = run(true, 1.0, &input);
        let pauses_in: Vec<f32> = (FRAME * 100..input.len())
            .filter(|&i| !voiced[i])
            .map(|i| input[i])
            .collect();
        let noise_in = rms_db(&pauses_in);
        let noise_out = pause_db(&out, &voiced);
        assert!(
            noise_in - noise_out > 25.0,
            "pause noise {noise_in:.1} -> {noise_out:.1} dB"
        );
        let t = shared.telemetry();
        assert_eq!(t.bad_frames, 0);
        assert!(t.snr_db.is_some() && t.vad.is_none());
        assert!(!t.overloaded);
    }

    #[test]
    fn higher_strength_removes_more() {
        let (input, voiced) = speech_and_fan(800, 5);
        let pause = |s: f32| pause_db(&run(true, s, &input).0, &voiced);
        let (low, mid, full) = (pause(0.2), pause(0.6), pause(1.0));
        assert!(
            low > mid && mid > full,
            "pause levels {low:.1} / {mid:.1} / {full:.1} dB"
        );
    }

    #[test]
    fn strength_mapping() {
        assert_eq!(strength_to_db(0.0), 0.0);
        assert!((strength_to_db(0.5) - 22.5).abs() < 1e-4);
        assert_eq!(strength_to_db(1.0), UNLIMITED_DB);
        assert_eq!(strength_to_db(-1.0), 0.0);
    }

    #[test]
    fn watchdog_trips_only_on_sustained_overload() {
        let mut w = Watchdog::new(7_000.0, 20);
        for _ in 0..19 {
            assert!(!w.update(9_000.0));
        }
        assert!(!w.update(1_000.0)); // one fast frame resets the count
        for _ in 0..19 {
            assert!(!w.update(9_000.0));
        }
        assert!(w.update(9_000.0));
    }

    #[test]
    fn overloaded_engine_passes_audio_through() {
        let mut r = rng(6);
        let input: Vec<f32> = (0..FRAME * 40).map(|_| r() * 0.3).collect();
        let shared = Arc::new(EngineShared::new(true, 1.0));
        shared.overloaded.store(true, Relaxed);
        let mut p = DeepFilterProcessor::new(shared);
        let mut out = Vec::new();
        for chunk in input.as_chunks::<FRAME>().0 {
            let mut f = *chunk;
            p.process(&mut f);
            out.extend_from_slice(&f);
        }
        assert_eq!(&out[DELAY..], &input[..input.len() - DELAY]);
    }

    /// The engine library is compiled optimized even in development builds (see Cargo.toml),
    /// so the overload watchdog must not trip during normal development use.
    #[test]
    fn runs_in_real_time_in_development_builds() {
        let (input, _) = speech_and_fan(300, 7);
        let shared = Arc::new(EngineShared::new(true, 1.0));
        let mut p = DeepFilterProcessor::new(shared.clone());
        for chunk in input.as_chunks::<FRAME>().0 {
            let mut f = *chunk;
            p.process(&mut f);
        }
        let t = shared.telemetry();
        assert!(
            !t.overloaded && t.avg_ms < 3.0,
            "{:.2} ms/frame, overloaded {}",
            t.avg_ms,
            t.overloaded
        );
    }
}

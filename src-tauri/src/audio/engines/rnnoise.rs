//! RNNoise engine (via `nnnoiseless`, a pure-Rust port of Xiph's RNNoise, BSD-3-Clause).
//!
//! - Format: 48 kHz mono, 480-sample frames, samples scaled to the 16-bit range (±32768).
//! - Algorithmic delay: one frame (480 samples, 10 ms), from RNNoise's overlap-add synthesis
//!   (verified by `delay_is_one_frame`).
//! - **Strength**: RNNoise has no strength parameter. Shuusshy implements strength as a
//!   documented *wet/dry mix*: `out = s * denoised + (1 - s) * original`, where `original` is
//!   delayed by the same one frame so the two are sample-aligned (no comb filtering).
//!   `s = 1` is full RNNoise; `s = 0` is the original audio. In dB terms, a mix limits the
//!   maximum attenuation to roughly `20·log10(1 - s)` (e.g. s = 0.8 → at most ~14 dB).
//! - **Bypass** (suppression off): the output is exactly the delayed original. The delay is kept
//!   in bypass so switching on/off never shifts the audio in time, and every change of
//!   on/off/strength is ramped over one frame so it cannot click.
//! - RNNoise keeps running while bypassed (≈1 % of a core) so its state is warm and switching on
//!   is instant, without the fade-in artifacts of a fresh state.

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use nnnoiseless::DenoiseState;

use super::EngineShared;
use crate::audio::pipeline::{FRAME, FrameProcessor};

const SCALE: f32 = 32768.0;

pub struct RnnoiseProcessor {
    state: Box<DenoiseState<'static>>,
    shared: Arc<EngineShared>,
    /// Previous input frame: the dry signal aligned with RNNoise's one-frame delay.
    prev: [f32; FRAME],
    scaled_in: [f32; FRAME],
    denoised: [f32; FRAME],
    /// Wet amount actually applied at the end of the last frame (ramped toward the target).
    wet: f32,
}

impl RnnoiseProcessor {
    /// Creates the engine (allocates; call off the audio thread) and primes it with one frame
    /// of silence, discarding RNNoise's documented first-frame fade-in artifacts.
    pub fn new(shared: Arc<EngineShared>) -> Self {
        let mut state = DenoiseState::new();
        let mut scratch = [0.0f32; FRAME];
        state.process_frame(&mut scratch, &[0.0; FRAME]);
        let wet = if shared.enabled() {
            shared.strength()
        } else {
            0.0
        };
        Self {
            state,
            shared,
            prev: [0.0; FRAME],
            scaled_in: [0.0; FRAME],
            denoised: [0.0; FRAME],
            wet,
        }
    }
}

impl FrameProcessor for RnnoiseProcessor {
    fn delay_frames(&self) -> usize {
        1
    }

    fn process(&mut self, frame: &mut [f32; FRAME]) {
        let started = Instant::now();

        for (o, &i) in self.scaled_in.iter_mut().zip(frame.iter()) {
            *o = i * SCALE;
        }
        let vad = self
            .state
            .process_frame(&mut self.denoised, &self.scaled_in);

        // A broken engine output must never reach the virtual microphone.
        let usable = self.denoised.iter().all(|v| v.is_finite());
        if !usable {
            self.shared.bad_frames.fetch_add(1, Relaxed);
        }

        let target = if self.shared.enabled() && usable {
            self.shared.strength()
        } else {
            0.0
        };
        let start = self.wet;
        let step = (target - start) / FRAME as f32;

        // Output: ramped mix of denoised (delayed by RNNoise) and the previous input frame.
        for i in 0..FRAME {
            let w = start + step * (i + 1) as f32;
            let dry = self.prev[i];
            let wet_sample = if usable {
                self.denoised[i] / SCALE
            } else {
                dry
            };
            let x = frame[i];
            self.prev[i] = x;
            frame[i] = (w * wet_sample + (1.0 - w) * dry).clamp(-1.0, 1.0);
        }
        self.wet = target;

        self.shared.vad_bits.store(vad.to_bits(), Relaxed);
        self.shared
            .record_frame(started.elapsed().as_secs_f32() * 1e6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// High-quality deterministic white noise (xorshift64*) in [-amp, amp].
    fn noise(len: usize, amp: f32, seed: u64) -> Vec<f32> {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..len)
            .map(|_| {
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40; // 24 bits
                (r as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * amp
            })
            .collect()
    }

    fn run_with(shared: Arc<EngineShared>, input: &[f32], switch_on_at: Option<usize>) -> Vec<f32> {
        let mut p = RnnoiseProcessor::new(shared.clone());
        let mut out = Vec::with_capacity(input.len());
        for (n, chunk) in input.as_chunks::<FRAME>().0.iter().enumerate() {
            if Some(n) == switch_on_at {
                shared.set(true, 1.0);
            }
            let mut f = *chunk;
            p.process(&mut f);
            out.extend_from_slice(&f);
        }
        out
    }

    fn run(enabled: bool, strength: f32, input: &[f32]) -> Vec<f32> {
        run_with(Arc::new(EngineShared::new(enabled, strength)), input, None)
    }

    fn rms_db(x: &[f32]) -> f32 {
        let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        10.0 * ms.max(1e-20).log10()
    }

    #[test]
    fn bypass_is_the_exact_input_delayed_by_one_frame() {
        let input = noise(FRAME * 50, 0.3, 1);
        let out = run(false, 0.8, &input);
        assert!(out[..FRAME].iter().all(|&v| v == 0.0));
        assert_eq!(&out[FRAME..], &input[..input.len() - FRAME]);
    }

    #[test]
    fn strength_zero_is_also_untouched() {
        let input = noise(FRAME * 20, 0.3, 2);
        let out = run(true, 0.0, &input);
        assert_eq!(&out[FRAME..], &input[..input.len() - FRAME]);
    }

    #[test]
    fn delay_is_one_frame() {
        // The denoised output stays correlated with the input; the best lag is RNNoise's
        // algorithmic delay, which the dry path must match exactly.
        let input = noise(FRAME * 200, 0.3, 3);
        let out = run(true, 1.0, &input);
        let skip = FRAME * 20;
        let corr = |lag: usize| -> f64 {
            (skip..input.len() - lag)
                .map(|i| out[i + lag] as f64 * input[i] as f64)
                .sum::<f64>()
        };
        let best = (0..2 * FRAME)
            .max_by(|&a, &b| corr(a).partial_cmp(&corr(b)).unwrap())
            .unwrap();
        assert_eq!(best, FRAME, "measured RNNoise delay {best} samples");
    }

    #[test]
    fn strength_is_an_exact_wet_dry_mix() {
        let input = noise(FRAME * 100, 0.05, 4);
        let full = run(true, 1.0, &input);
        let dry = run(true, 0.0, &input);
        for s in [0.25f32, 0.5, 0.8] {
            let mixed = run(true, s, &input);
            for i in FRAME..input.len() {
                let expect = s * full[i] + (1.0 - s) * dry[i];
                assert!((mixed[i] - expect).abs() < 1e-5, "strength {s}, sample {i}");
            }
        }
    }

    #[test]
    fn switching_on_is_a_one_frame_linear_crossfade() {
        let input = noise(FRAME * 40, 0.2, 5);
        let full = run(true, 1.0, &input);
        let dry = run(false, 1.0, &input);
        let switched = run_with(Arc::new(EngineShared::new(false, 1.0)), &input, Some(20));
        // Before: untouched. During frame 20: linear ramp. After: full suppression.
        assert_eq!(&switched[..20 * FRAME], &dry[..20 * FRAME]);
        for i in 0..FRAME {
            let w = (i + 1) as f32 / FRAME as f32;
            let k = 20 * FRAME + i;
            let expect = w * full[k] + (1.0 - w) * dry[k];
            assert!((switched[k] - expect).abs() < 1e-5, "ramp sample {i}");
        }
        assert_eq!(&switched[21 * FRAME..], &full[21 * FRAME..]);
    }

    #[test]
    fn input_scaling_reaches_rnnoise() {
        // If samples were not scaled to the 16-bit range, RNNoise would see near-silence and
        // pass audio through unchanged. With correct scaling it clearly attenuates noise.
        for seed in [10, 11, 12] {
            let input = noise(FRAME * 300, 0.05, seed);
            let out = run(true, 1.0, &input);
            let tail = FRAME * 100;
            let reduction = rms_db(&input[tail..]) - rms_db(&out[tail..]);
            assert!(reduction > 3.0, "seed {seed}: only {reduction:.1} dB");
        }
    }

    #[test]
    fn output_is_always_finite_and_bounded() {
        let mut input = noise(FRAME * 20, 1.0, 7);
        input[FRAME * 5] = 1.0;
        let out = run(true, 1.0, &input);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    }

    #[test]
    fn telemetry_counts_frames() {
        let shared = Arc::new(EngineShared::new(true, 1.0));
        run_with(shared.clone(), &noise(FRAME * 12, 0.1, 8), None);
        let t = shared.telemetry();
        assert_eq!((t.frames, t.bad_frames), (12, 0));
        assert!((0.0..=1.0).contains(&t.vad.unwrap()));
        assert_eq!(t.snr_db, None);
    }
}

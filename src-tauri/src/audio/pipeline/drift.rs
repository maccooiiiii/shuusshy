//! Clock-drift compensation.
//!
//! The microphone and the VB-CABLE playback endpoint are driven by independent clocks. Even when
//! both claim 48 kHz, one runs slightly faster (tens to ~100 ppm measured on the development PC),
//! so a buffer between them slowly fills up (growing latency) or drains (dropouts).
//!
//! **What is measured.** VB-CABLE does not pull audio smoothly: it takes ~20 ms per callback,
//! sometimes late, so the output buffer level is a sawtooth with irregular teeth. Two earlier
//! designs failed on hardware and are recorded in docs/AUDIO_ENGINE.md:
//! - steering the *instantaneous* level chased the sawtooth (±1700 ppm swings);
//! - steering the *post-pull low-water mark* to a thin 10 ms margin underran on ordinary jitter.
//!
//! - steering a one-second average of the level, while re-priming to a different reference
//!   point, made every underrun push the average up and caused a correction/underrun spiral.
//!
//! The output callback therefore measures one quantity, the **margin left in the buffer right
//! after each pull**, and everything is defined on it: playback (re)starts only once a pull would
//! leave the target margin, resyncs trim back to it, and once per second the DSP thread feeds its
//! **one-second average** to this PI controller (critically damped, ~30 s response), which learns
//! the real clock difference without reacting to individual late pulls.
//!
//! Transients (a stall followed by a burst of backlog) are kept away from the drift estimate:
//! the integral only accumulates small errors, the proportional response is capped, and large
//! excesses are trimmed by the output callback's resync instead.
//!
//! The correction is applied by the output resampler and is inaudible (a few hundred ppm at
//! most in practice; 1000 ppm ≈ 1.7 cents).

/// Largest correction the controller may apply, in ppm.
pub const MAX_CORRECTION_PPM: f64 = 2000.0;

#[derive(Debug, Clone)]
pub struct DriftController {
    target: f64,
    integral: f64,
    /// ppm per sample of error.
    kp: f64,
    /// ppm per (sample · second) of error.
    ki: f64,
    correction_ppm: f64,
}

impl DriftController {
    /// `target` is the desired average post-pull margin, in output-rate samples.
    pub fn new(target: usize, sample_rate: u32) -> Self {
        // Plant: a correction of 1 ppm changes the buffer by `g` samples per second.
        let g = sample_rate as f64 * 1e-6;
        // PI tuned for a critically damped response (zeta = 1) with natural frequency w:
        //   Kp * g = 2w,  Ki * g = w^2.  w = 1/30 s gives a ~30 s settling time.
        let w = 1.0 / 30.0;
        Self {
            target: target as f64,
            integral: 0.0,
            kp: 2.0 * w / g,
            ki: w * w / g,
            correction_ppm: 0.0,
        }
    }

    /// Feeds the average post-pull margin over the last `dt` seconds and returns the relative
    /// resampling ratio to apply (`1.0` = nominal; `< 1.0` produces fewer samples).
    pub fn update(&mut self, average_margin: f64, dt: f64) -> f64 {
        let err = average_margin - self.target;
        // Cap the immediate reaction to one target's worth of error.
        let p = self.kp * err.clamp(-self.target, self.target);
        // Conditional integration: only errors within half the target reflect slow clock drift;
        // larger ones are transients (stall + burst) and would wind up a false drift estimate.
        if err.abs() <= self.target * 0.5 {
            let candidate = self.integral + self.ki * err * dt;
            // Anti-windup: integrate only while unsaturated, or when moving back toward zero.
            if (p + candidate).abs() < MAX_CORRECTION_PPM || candidate.abs() < self.integral.abs() {
                self.integral = candidate;
            }
        }
        // Too full -> produce fewer samples -> negative correction.
        self.correction_ppm = (-(p + self.integral)).clamp(-MAX_CORRECTION_PPM, MAX_CORRECTION_PPM);
        1.0 + self.correction_ppm / 1e6
    }

    pub fn correction_ppm(&self) -> f64 {
        self.correction_ppm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARGIN: f64 = 960.0; // 20 ms at 48 kHz
    const MAX_MARGIN: f64 = 2880.0; // 60 ms

    struct Sim {
        underruns: usize,
        resyncs: usize,
        corrections: Vec<f64>,
        margins: Vec<f64>,
        min_margin: f64,
    }

    /// Models the timing measured on the development PC: the microphone delivers one 10 ms frame
    /// per tick (its clock `drift_ppm` off), but 5% of packets arrive a tick late and then two
    /// arrive together; VB-CABLE pulls 20 ms every other tick, but 5% of pulls come a tick late
    /// and take the backlog at once. Priming and margin measurement mirror `io.rs`.
    fn simulate(drift_ppm: f64, seconds: usize, seed: u32) -> (DriftController, Sim) {
        simulate_with_stalls(drift_ppm, seconds, seed, 0)
    }

    /// As [`simulate`], plus a system stall every `stall_every` seconds (if non-zero): neither
    /// side runs for 50 ms, then the backlog arrives at once, like the stalls seen on hardware.
    fn simulate_with_stalls(
        drift_ppm: f64,
        seconds: usize,
        seed: u32,
        stall_every: usize,
    ) -> (DriftController, Sim) {
        let mut c = DriftController::new(MARGIN as usize, 48_000);
        let mut fill = 0.0;
        let mut priming = true;
        let mut ratio = 1.0;
        let mut rng = seed;
        let mut rand = || {
            rng = rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (rng >> 16) as f64 / 65_536.0
        };
        let mut sim = Sim {
            underruns: 0,
            resyncs: 0,
            corrections: Vec::new(),
            margins: Vec::new(),
            min_margin: f64::MAX,
        };
        let (mut pending_in, mut pending_out) = (0.0, 0.0);
        for sec in 0..seconds {
            let (mut sum, mut n) = (0.0, 0);
            let stalled = |tick: usize| {
                stall_every > 0 && sec > 0 && sec % stall_every == 0 && (40..45).contains(&tick)
            };
            for tick in 0..100 {
                pending_in += 480.0 * (1.0 + drift_ppm / 1e6) * ratio;
                if tick % 2 == 1 {
                    pending_out += 960.0;
                }
                if stalled(tick) {
                    continue;
                }
                // A packet is at most one tick late (as measured outside stalls).
                if pending_in > 600.0 || rand() > 0.05 {
                    fill += pending_in;
                    pending_in = 0.0;
                }
                if pending_out > 0.0 && rand() > 0.05 {
                    let request = pending_out;
                    pending_out = 0.0;
                    if priming {
                        // Plays silence until a pull would leave the target margin.
                        if fill >= MARGIN + request {
                            priming = false;
                        } else {
                            continue;
                        }
                    }
                    let mut margin = fill - request;
                    if margin > MAX_MARGIN {
                        margin = MARGIN; // resync: discard the excess
                        sim.resyncs += 1;
                    }
                    if margin < 0.0 {
                        sim.underruns += 1;
                        fill = 0.0;
                        priming = true;
                    } else {
                        fill = margin;
                        sum += margin;
                        n += 1;
                        if sec > 60 {
                            sim.min_margin = sim.min_margin.min(margin);
                        }
                    }
                }
            }
            if n > 0 {
                let avg = sum / n as f64;
                ratio = c.update(avg, 1.0);
                sim.margins.push(avg);
                sim.corrections.push(c.correction_ppm());
            }
        }
        (c, sim)
    }

    #[test]
    fn learns_real_world_drift_without_underruns() {
        for drift in [-200.0, -100.0, -37.0, 0.0, 60.0, 150.0] {
            let (_, sim) = simulate(drift, 900, 7);
            // Per-second readings carry ~±35 ppm of measurement noise; the mean over the last
            // five minutes must match the true clock difference.
            let learned = mean(&sim.corrections[sim.corrections.len() - 300..]);
            assert!(
                (learned + drift).abs() < 15.0,
                "drift {drift}: learned correction {learned:.1}"
            );
            assert_eq!(sim.underruns, 0, "drift {drift}: underruns");
            let avg = mean(&sim.margins[sim.margins.len() - 300..]);
            assert!(
                (avg - MARGIN).abs() < 48.0,
                "drift {drift}: average margin {avg:.0}"
            );
            assert_eq!(sim.resyncs, 0, "drift {drift}: resyncs without stalls");
            // Even the worst late packet/pull combination left some margin.
            assert!(
                sim.min_margin > 0.0,
                "drift {drift}: min margin {}",
                sim.min_margin
            );
        }
    }

    #[test]
    fn correction_stays_calm_with_jittery_timing() {
        for seed in [1, 99, 12345] {
            let (_, sim) = simulate(0.0, 600, seed);
            let tail = &sim.corrections[sim.corrections.len() - 300..];
            let worst = tail.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            assert!(worst < 120.0, "seed {seed}: worst correction {worst:.1}");
            assert!(
                mean(tail).abs() < 15.0,
                "seed {seed}: mean {:.1}",
                mean(tail)
            );
            assert_eq!(sim.underruns, 0);
        }
    }

    #[test]
    fn recovers_from_system_stalls_without_corrupting_the_drift_estimate() {
        let (_, sim) = simulate_with_stalls(-37.0, 900, 5, 60);
        // A 50 ms stall exceeds the 20 ms margin, so each may cost one dropout, but no more.
        assert!(sim.underruns <= 15, "underruns {}", sim.underruns);
        let learned = mean(&sim.corrections[sim.corrections.len() - 300..]);
        assert!(
            (learned - 37.0).abs() < 25.0,
            "learned correction {learned:.1}"
        );
        let avg = mean(&sim.margins[sim.margins.len() - 300..]);
        assert!((avg - MARGIN).abs() < 120.0, "average margin {avg:.0}");
    }

    fn mean(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() / v.len() as f64
    }

    #[test]
    fn correction_is_bounded_and_points_the_right_way() {
        let mut c = DriftController::new(960, 48_000);
        for _ in 0..1000 {
            c.update(48_000.0, 1.0); // far too full
        }
        assert!(c.correction_ppm() < -500.0 && c.correction_ppm() >= -MAX_CORRECTION_PPM);
        let mut c = DriftController::new(960, 48_000);
        for _ in 0..1000 {
            c.update(0.0, 1.0); // empty
        }
        assert!(c.correction_ppm() > 500.0 && c.correction_ppm() <= MAX_CORRECTION_PPM);
        // Small persistent errors are integrated up to the bound, never past it.
        let mut c = DriftController::new(960, 48_000);
        for _ in 0..100_000 {
            c.update(1300.0, 1.0);
        }
        assert!(c.correction_ppm() >= -MAX_CORRECTION_PPM - 1e-9);
    }
}

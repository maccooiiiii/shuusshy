//! Processing chain around the noise-suppression engine (Milestone 6):
//!
//! ```text
//! input gain → engine (RNNoise / DeepFilterNet) → noise gate → output gain → safety limiter
//! ```
//!
//! - **Gains** are in dB (±24) and ramp linearly over one frame when changed, so they never click.
//!   Input gain sits before the engine, so a quiet microphone is brought up before suppression.
//! - **Noise gate** (optional) works on the engine's output, per 10 ms frame:
//!   opens when the frame level exceeds the threshold; closes only after the level has stayed
//!   4 dB below the threshold (hysteresis) for 200 ms (hold). The gain follows smoothly: about
//!   2 ms to open, 50 ms to close, down to −80 dB. A disabled gate is exactly transparent.
//! - **Voice isolation** (optional) reuses the gate with an *adaptive* threshold: it tracks the
//!   user's own voice level (the upper envelope of the engine output's frame levels: rising at
//!   10 % of the difference per frame, so one loud bump barely moves it, and decaying 1 dB/s)
//!   and keeps the gate threshold `margin` dB below it. A nearby speaker is much louder at
//!   their own microphone than a background talker, so background speech is muted while the
//!   user pauses. It cannot separate voices that talk at the same time.
//! - **Look-ahead without extra delay**: the engines delay audio (RNNoise 1 frame,
//!   DeepFilterNet 3 frames), so the engine *input* shows what the output will contain a few
//!   frames from now. The gate opens early when that upcoming input is speech (a separate
//!   RNNoise instance used only as a voice-activity detector) **and** at least 6 dB above the
//!   threshold, so word onsets
//!   after a pause are not clipped while keyboard clicks and fan noise in the raw input cannot
//!   open it. The detector only runs while the gate or voice isolation is enabled. Closing
//!   still follows the cleaned output.
//! - **Safety limiter**: a peak limiter at −0.5 dBFS (instant attack, 100 ms release) so a high
//!   output gain cannot clip the virtual microphone. Below the threshold it is exactly
//!   transparent. Frames where it reduces gain by more than 1 dB are counted.
//!
//! **Meters and monitoring (Milestone 7).** The chain measures real RMS and peak levels of the
//! audio *before* suppression (after input gain) and of the final output, every frame, with
//! meter ballistics (RMS integrated over ~300 ms, peak hold falling 15 dB/s). For the live test
//! it can also copy either signal into a monitor ring: "original" is delayed by the engine's own
//! delay so switching between original and processed never jumps in time. Monitor rings are
//! handed in and out through lock-free mailboxes so the audio thread never allocates or frees.
//!
//! Gains and the gate are separate user controls: turning noise cancellation off bypasses the
//! engine only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering::Relaxed};

use serde::Serialize;

use nnnoiseless::DenoiseState;
use rtrb::{Consumer, Producer};

use super::pipeline::{FRAME, FrameProcessor, PROCESS_RATE};

const GATE_HYSTERESIS_DB: f32 = 4.0;
const GATE_HOLD_FRAMES: u32 = 20; // 200 ms
const GATE_FLOOR: f32 = 1e-4; // -80 dB
const GATE_ATTACK_S: f32 = 0.002;
const GATE_RELEASE_S: f32 = 0.050;
const LIMIT: f32 = 0.944; // -0.5 dBFS
/// Monitor sources (what the live test plays to headphones).
pub const MONITOR_OFF: u8 = 0;
pub const MONITOR_ORIGINAL: u8 = 1;
pub const MONITOR_PROCESSED: u8 = 2;
/// Meter floor (dBFS) and ballistics.
pub const METER_FLOOR_DB: f32 = -100.0;
const METER_RMS_S: f32 = 0.3;
const METER_PEAK_FALL_DB_PER_S: f32 = 15.0;

/// Most engine-delay frames the gate can look ahead over.
const LOOKAHEAD_MAX: usize = 8;
/// Voice-activity probability above which upcoming input counts as speech.
const LOOKAHEAD_VAD: f32 = 0.6;
/// Upcoming (raw, still noisy) speech must exceed the threshold by this much to open the gate
/// early, so background speech plus noise in the raw input cannot open it.
const LOOKAHEAD_HEADROOM_DB: f32 = 6.0;
/// Voice-level tracker: share of the gap closed per frame when rising, dB lost per frame when
/// falling (1 dB/s), starting point, and the quietest level it treats as a voice.
const VOICE_RISE: f32 = 0.1;
const VOICE_DECAY_DB: f32 = 0.01;
const VOICE_START_DB: f32 = -70.0;
const VOICE_MIN_DB: f32 = -60.0;
const LIMITER_RELEASE_S: f32 = 0.100;

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Live level controls and telemetry, shared lock-free with the controller and UI.
#[derive(Debug)]
pub struct ChainShared {
    pub input_gain_db_bits: AtomicU32,
    pub output_gain_db_bits: AtomicU32,
    pub gate_enabled: AtomicBool,
    pub gate_threshold_db_bits: AtomicU32,
    pub isolation_enabled: AtomicBool,
    pub isolation_margin_db_bits: AtomicU32,
    /// Telemetry: tracked voice level (dBFS, `f32` bits).
    pub voice_level_db_bits: AtomicU32,
    /// What the monitor ring receives: `MONITOR_OFF`, `MONITOR_ORIGINAL` or `MONITOR_PROCESSED`.
    pub monitor_source: AtomicU8,
    /// Meters (dBFS, `f32` bits): before suppression and final output.
    pub in_rms_db_bits: AtomicU32,
    pub in_peak_db_bits: AtomicU32,
    pub out_rms_db_bits: AtomicU32,
    pub out_peak_db_bits: AtomicU32,
    /// Telemetry: gate currently open (true also when the gate is disabled).
    pub gate_open: AtomicBool,
    /// Telemetry: frames in which the safety limiter reduced gain by more than 1 dB.
    pub limited_frames: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    pub input_gain_db: f32,
    pub output_gain_db: f32,
    pub gate_enabled: bool,
    pub gate_threshold_db: f32,
    /// Voice isolation: adaptive gate keyed to the user's own voice level.
    pub isolation_enabled: bool,
    /// How far below the tracked voice level the isolation threshold sits (dB).
    pub isolation_margin_db: f32,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            input_gain_db: 0.0,
            output_gain_db: 0.0,
            gate_enabled: false,
            gate_threshold_db: -50.0,
            isolation_enabled: false,
            isolation_margin_db: 12.0,
        }
    }
}

/// Real-time levels for the meters, in dBFS (`METER_FLOOR_DB` = silence).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meters {
    pub input_rms_db: f32,
    pub input_peak_db: f32,
    pub output_rms_db: f32,
    pub output_peak_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainTelemetry {
    pub gate_enabled: bool,
    pub gate_open: bool,
    pub limited_frames: u64,
    pub isolation_enabled: bool,
    /// Tracked voice level in dBFS (meaningful once the user has spoken).
    pub voice_level_db: f32,
}

impl ChainShared {
    pub fn new(levels: Levels) -> Self {
        let s = Self {
            input_gain_db_bits: AtomicU32::new(0),
            output_gain_db_bits: AtomicU32::new(0),
            gate_enabled: AtomicBool::new(false),
            gate_threshold_db_bits: AtomicU32::new(0),
            isolation_enabled: AtomicBool::new(false),
            isolation_margin_db_bits: AtomicU32::new(0),
            voice_level_db_bits: AtomicU32::new(VOICE_START_DB.to_bits()),
            monitor_source: AtomicU8::new(MONITOR_OFF),
            in_rms_db_bits: AtomicU32::new(METER_FLOOR_DB.to_bits()),
            in_peak_db_bits: AtomicU32::new(METER_FLOOR_DB.to_bits()),
            out_rms_db_bits: AtomicU32::new(METER_FLOOR_DB.to_bits()),
            out_peak_db_bits: AtomicU32::new(METER_FLOOR_DB.to_bits()),
            gate_open: AtomicBool::new(true),
            limited_frames: AtomicU64::new(0),
        };
        s.set(levels);
        s
    }

    pub fn set(&self, l: Levels) {
        self.input_gain_db_bits
            .store(l.input_gain_db.to_bits(), Relaxed);
        self.output_gain_db_bits
            .store(l.output_gain_db.to_bits(), Relaxed);
        self.gate_enabled.store(l.gate_enabled, Relaxed);
        self.gate_threshold_db_bits
            .store(l.gate_threshold_db.to_bits(), Relaxed);
        self.isolation_enabled.store(l.isolation_enabled, Relaxed);
        self.isolation_margin_db_bits
            .store(l.isolation_margin_db.to_bits(), Relaxed);
    }

    fn load(&self) -> Levels {
        let f = |a: &AtomicU32| f32::from_bits(a.load(Relaxed));
        Levels {
            input_gain_db: f(&self.input_gain_db_bits),
            output_gain_db: f(&self.output_gain_db_bits),
            gate_enabled: self.gate_enabled.load(Relaxed),
            gate_threshold_db: f(&self.gate_threshold_db_bits),
            isolation_enabled: self.isolation_enabled.load(Relaxed),
            isolation_margin_db: f(&self.isolation_margin_db_bits),
        }
    }

    pub fn reset_run(&self) {
        self.gate_open.store(true, Relaxed);
        self.limited_frames.store(0, Relaxed);
        self.voice_level_db_bits
            .store(VOICE_START_DB.to_bits(), Relaxed);
        self.monitor_source.store(MONITOR_OFF, Relaxed);
        for a in [
            &self.in_rms_db_bits,
            &self.in_peak_db_bits,
            &self.out_rms_db_bits,
            &self.out_peak_db_bits,
        ] {
            a.store(METER_FLOOR_DB.to_bits(), Relaxed);
        }
    }

    pub fn meters(&self) -> Meters {
        let f = |a: &AtomicU32| f32::from_bits(a.load(Relaxed));
        Meters {
            input_rms_db: f(&self.in_rms_db_bits),
            input_peak_db: f(&self.in_peak_db_bits),
            output_rms_db: f(&self.out_rms_db_bits),
            output_peak_db: f(&self.out_peak_db_bits),
        }
    }

    pub fn telemetry(&self) -> ChainTelemetry {
        ChainTelemetry {
            gate_enabled: self.gate_enabled.load(Relaxed),
            gate_open: self.gate_open.load(Relaxed),
            limited_frames: self.limited_frames.load(Relaxed),
            isolation_enabled: self.isolation_enabled.load(Relaxed),
            voice_level_db: f32::from_bits(self.voice_level_db_bits.load(Relaxed)),
        }
    }
}

/// Frame-decided, sample-smoothed noise gate.
#[derive(Debug, Clone)]
pub struct Gate {
    open: bool,
    below_frames: u32,
    gain: f32,
    attack: f32,
    release: f32,
}

impl Default for Gate {
    fn default() -> Self {
        let coef = |s: f32| 1.0 - (-1.0 / (s * PROCESS_RATE as f32)).exp();
        Self {
            open: true,
            below_frames: 0,
            gain: 1.0,
            attack: coef(GATE_ATTACK_S),
            release: coef(GATE_RELEASE_S),
        }
    }
}

impl Gate {
    /// Applies the gate in place. When disabled it is transparent (the gain still glides back to
    /// unity if the gate was closed when it was switched off).
    /// `upcoming_db` is the level of audio that will reach this point within the next few
    /// frames (look-ahead); pass `f32::NEG_INFINITY` when unknown.
    pub fn process(
        &mut self,
        frame: &mut [f32; FRAME],
        enabled: bool,
        threshold_db: f32,
        upcoming_db: f32,
    ) {
        if enabled {
            let ms = frame.iter().map(|v| v * v).sum::<f32>() / FRAME as f32;
            let level_db = 10.0 * ms.max(1e-12).log10();
            if level_db > threshold_db || upcoming_db > threshold_db + LOOKAHEAD_HEADROOM_DB {
                self.open = true;
                self.below_frames = 0;
            } else if level_db < threshold_db - GATE_HYSTERESIS_DB {
                self.below_frames += 1;
                if self.below_frames >= GATE_HOLD_FRAMES {
                    self.open = false;
                }
            } else {
                // Between threshold and hysteresis: keep the current state, restart the hold.
                self.below_frames = 0;
            }
        } else {
            self.open = true;
            self.below_frames = 0;
        }
        let target = if self.open { 1.0 } else { GATE_FLOOR };
        if self.gain == 1.0 && target == 1.0 {
            return; // exactly transparent
        }
        let coef = if target > self.gain {
            self.attack
        } else {
            self.release
        };
        for v in frame.iter_mut() {
            self.gain += (target - self.gain) * coef;
            if target == 1.0 && self.gain > 0.9999 {
                self.gain = 1.0;
            }
            *v *= self.gain;
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

/// Tracks the user's voice level: the upper envelope of frame levels, rising gradually and
/// decaying slowly.
#[derive(Debug, Clone)]
pub struct VoiceTracker {
    level_db: f32,
}

impl Default for VoiceTracker {
    fn default() -> Self {
        Self {
            level_db: VOICE_START_DB,
        }
    }
}

impl VoiceTracker {
    pub fn update(&mut self, frame_db: f32) -> f32 {
        if frame_db > self.level_db && frame_db > VOICE_MIN_DB {
            self.level_db += (frame_db - self.level_db) * VOICE_RISE;
        } else {
            self.level_db = (self.level_db - VOICE_DECAY_DB).max(VOICE_START_DB);
        }
        self.level_db
    }
}

fn frame_db(frame: &[f32; FRAME]) -> f32 {
    let ms = frame.iter().map(|v| v * v).sum::<f32>() / FRAME as f32;
    10.0 * ms.max(1e-12).log10()
}

/// Meter ballistics for one signal: RMS integrated over ~300 ms, peak with a falling hold.
#[derive(Debug, Clone)]
pub struct Meter {
    power: f32,
    peak: f32,
    rms_coef: f32,
    peak_fall: f32,
}

impl Default for Meter {
    fn default() -> Self {
        let frame_s = FRAME as f32 / PROCESS_RATE as f32;
        Self {
            power: 0.0,
            peak: 0.0,
            rms_coef: 1.0 - (-frame_s / METER_RMS_S).exp(),
            peak_fall: 10f32.powf(-METER_PEAK_FALL_DB_PER_S * frame_s / 20.0),
        }
    }
}

impl Meter {
    /// Feeds one frame; returns (rms dBFS, peak dBFS).
    pub fn update(&mut self, frame: &[f32; FRAME]) -> (f32, f32) {
        let ms = frame.iter().map(|v| v * v).sum::<f32>() / FRAME as f32;
        let pk = frame.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        self.power += (ms - self.power) * self.rms_coef;
        self.peak = pk.max(self.peak * self.peak_fall);
        let to_db = |x: f32, scale: f32| (scale * x.max(1e-12).log10()).max(METER_FLOOR_DB);
        (to_db(self.power, 10.0), to_db(self.peak, 20.0))
    }
}

/// Lock-free hand-over of monitor rings to and from the audio thread. The controller keeps
/// this; the chain gets the other ends. New rings arrive through `inbox`; replaced ones go back
/// through `returned` so they are freed off the audio thread.
pub struct MonitorLink {
    pub inbox: Producer<Producer<f32>>,
    pub returned: Consumer<Producer<f32>>,
}

/// The chain's ends of a [`MonitorLink`].
pub struct MonitorPort {
    inbox: Consumer<Producer<f32>>,
    returned: Producer<Producer<f32>>,
}

/// Creates a connected link (controller side) and port (chain side).
pub fn monitor_link() -> (MonitorLink, MonitorPort) {
    let (inbox_tx, inbox_rx) = rtrb::RingBuffer::new(2);
    let (ret_tx, ret_rx) = rtrb::RingBuffer::new(4);
    (
        MonitorLink {
            inbox: inbox_tx,
            returned: ret_rx,
        },
        MonitorPort {
            inbox: inbox_rx,
            returned: ret_tx,
        },
    )
}

/// Peak limiter: instant attack, exponential release; transparent below the threshold.
#[derive(Debug, Clone)]
pub struct Limiter {
    env: f32,
    release: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Self {
            env: 0.0,
            release: (-1.0 / (LIMITER_RELEASE_S * PROCESS_RATE as f32)).exp(),
        }
    }
}

impl Limiter {
    /// Returns the largest gain reduction applied in this frame, in dB (0 = untouched).
    pub fn process(&mut self, frame: &mut [f32; FRAME]) -> f32 {
        let mut min_gain = 1.0f32;
        for v in frame.iter_mut() {
            let a = v.abs();
            self.env = if a > self.env {
                a
            } else {
                self.env * self.release
            };
            if self.env > LIMIT {
                let g = LIMIT / self.env;
                *v *= g;
                min_gain = min_gain.min(g);
            }
        }
        -20.0 * min_gain.log10()
    }
}

/// Wraps an engine with gains, gate and limiter.
pub struct ProcessingChain {
    engine: Box<dyn FrameProcessor>,
    shared: Arc<ChainShared>,
    gate: Gate,
    voice: VoiceTracker,
    in_meter: Meter,
    out_meter: Meter,
    monitor_port: Option<MonitorPort>,
    monitor_out: Option<Producer<f32>>,
    /// Recent engine-input frames, to delay the "original" monitor signal like the engine.
    original: [[f32; FRAME]; LOOKAHEAD_MAX + 1],
    original_pos: usize,
    /// Levels of the most recent engine-input frames that were speech (ring; −∞ otherwise),
    /// for the gate's look-ahead.
    input_levels: [f32; LOOKAHEAD_MAX],
    /// Voice-activity detector on the engine input (RNNoise used only for its VAD output).
    detector: Box<DenoiseState<'static>>,
    detector_in: [f32; FRAME],
    detector_out: [f32; FRAME],
    input_pos: usize,
    lookahead: usize,
    limiter: Limiter,
    in_gain: f32,
    out_gain: f32,
}

impl ProcessingChain {
    pub fn new(engine: Box<dyn FrameProcessor>, shared: Arc<ChainShared>) -> Self {
        let l = shared.load();
        let lookahead = engine.delay_frames().min(LOOKAHEAD_MAX);
        Self {
            engine,
            shared,
            lookahead,
            gate: Gate::default(),
            voice: VoiceTracker::default(),
            input_levels: [f32::NEG_INFINITY; LOOKAHEAD_MAX],
            in_meter: Meter::default(),
            out_meter: Meter::default(),
            monitor_port: None,
            monitor_out: None,
            original: [[0.0; FRAME]; LOOKAHEAD_MAX + 1],
            original_pos: 0,
            detector: DenoiseState::new(),
            detector_in: [0.0; FRAME],
            detector_out: [0.0; FRAME],
            input_pos: 0,
            limiter: Limiter::default(),
            in_gain: db_to_gain(l.input_gain_db),
            out_gain: db_to_gain(l.output_gain_db),
        }
    }
}

impl ProcessingChain {
    /// Connects the chain's end of a monitor link (see [`monitor_link`]).
    pub fn with_monitor(mut self, port: MonitorPort) -> Self {
        self.monitor_port = Some(port);
        self
    }
}

/// Multiplies by a gain ramping linearly from `from` to `to` across the frame.
fn ramp_gain(frame: &mut [f32; FRAME], from: f32, to: f32) {
    if from == 1.0 && to == 1.0 {
        return;
    }
    let step = (to - from) / FRAME as f32;
    for (i, v) in frame.iter_mut().enumerate() {
        *v *= from + step * (i + 1) as f32;
    }
}

impl FrameProcessor for ProcessingChain {
    fn process(&mut self, frame: &mut [f32; FRAME]) {
        let l = self.shared.load();

        let target_in = db_to_gain(l.input_gain_db);
        ramp_gain(frame, self.in_gain, target_in);
        self.in_gain = target_in;

        // Meter and remember the audio before suppression (after input gain).
        let (rms, peak) = self.in_meter.update(frame);
        self.shared.in_rms_db_bits.store(rms.to_bits(), Relaxed);
        self.shared.in_peak_db_bits.store(peak.to_bits(), Relaxed);
        self.original[self.original_pos] = *frame;
        let delayed_original =
            (self.original_pos + LOOKAHEAD_MAX + 1 - self.lookahead) % (LOOKAHEAD_MAX + 1);
        self.original_pos = (self.original_pos + 1) % (LOOKAHEAD_MAX + 1);

        // Remember the engine input level if it is speech: it reaches the output `lookahead`
        // frames later. The detector only runs when a gate is in use.
        let gating = l.gate_enabled || l.isolation_enabled;
        self.input_levels[self.input_pos] = if gating && self.lookahead > 0 {
            for (o, &v) in self.detector_in.iter_mut().zip(frame.iter()) {
                *o = v * 32768.0;
            }
            let vad = self
                .detector
                .process_frame(&mut self.detector_out, &self.detector_in);
            if vad > LOOKAHEAD_VAD {
                frame_db(frame)
            } else {
                f32::NEG_INFINITY
            }
        } else {
            f32::NEG_INFINITY
        };
        self.input_pos = (self.input_pos + 1) % LOOKAHEAD_MAX;
        let upcoming_db = (1..=self.lookahead)
            .map(|k| self.input_levels[(self.input_pos + LOOKAHEAD_MAX - k) % LOOKAHEAD_MAX])
            .fold(f32::NEG_INFINITY, f32::max);

        self.engine.process(frame);

        // The voice level is tracked continuously so isolation is ready the moment it is enabled.
        let voice_db = self.voice.update(frame_db(frame));
        self.shared
            .voice_level_db_bits
            .store(voice_db.to_bits(), Relaxed);
        let mut threshold = f32::NEG_INFINITY;
        if l.gate_enabled {
            threshold = l.gate_threshold_db;
        }
        if l.isolation_enabled {
            threshold = threshold.max(voice_db - l.isolation_margin_db);
        }
        self.gate.process(frame, gating, threshold, upcoming_db);
        self.shared.gate_open.store(self.gate.is_open(), Relaxed);

        let target_out = db_to_gain(l.output_gain_db);
        ramp_gain(frame, self.out_gain, target_out);
        self.out_gain = target_out;

        if self.limiter.process(frame) > 1.0 {
            self.shared.limited_frames.fetch_add(1, Relaxed);
        }
        for v in frame.iter_mut() {
            *v = v.clamp(-1.0, 1.0);
        }

        let (rms, peak) = self.out_meter.update(frame);
        self.shared.out_rms_db_bits.store(rms.to_bits(), Relaxed);
        self.shared.out_peak_db_bits.store(peak.to_bits(), Relaxed);

        // Monitor: pick up a newly handed-in ring (returning the old one), then copy the chosen
        // signal. A full ring only drops monitor audio, never virtual-microphone audio.
        if let Some(port) = &mut self.monitor_port
            && let Ok(ring) = port.inbox.pop()
            && let Some(old) = self.monitor_out.replace(ring)
        {
            let _ = port.returned.push(old);
        }
        if let Some(out) = &mut self.monitor_out {
            let source = match self.shared.monitor_source.load(Relaxed) {
                MONITOR_ORIGINAL => Some(&self.original[delayed_original]),
                MONITOR_PROCESSED => Some(&*frame),
                _ => None,
            };
            if let Some(src) = source
                && let Ok(chunk) = out.write_chunk_uninit(FRAME)
            {
                chunk.fill_from_iter(src.iter().copied());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::pipeline::Passthrough;

    fn sine(frames: usize, amp: f32, hz: f32) -> Vec<f32> {
        (0..frames * FRAME)
            .map(|i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / 48_000.0).sin())
            .collect()
    }

    fn rms_db(x: &[f32]) -> f32 {
        let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32;
        10.0 * ms.max(1e-20).log10()
    }

    fn chain(levels: Levels) -> (ProcessingChain, Arc<ChainShared>) {
        let shared = Arc::new(ChainShared::new(levels));
        (
            ProcessingChain::new(Box::new(Passthrough), shared.clone()),
            shared,
        )
    }

    fn run(c: &mut ProcessingChain, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::with_capacity(input.len());
        for chunk in input.as_chunks::<FRAME>().0 {
            let mut f = *chunk;
            c.process(&mut f);
            out.extend_from_slice(&f);
        }
        out
    }

    #[test]
    fn default_chain_is_exactly_transparent() {
        let input = sine(50, 0.5, 440.0);
        let (mut c, _) = chain(Levels::default());
        assert_eq!(run(&mut c, &input), input);
    }

    #[test]
    fn gains_apply_in_db() {
        let input = sine(50, 0.1, 440.0);
        for (gin, gout, expect) in [(6.0, 0.0, 6.0), (0.0, -12.0, -12.0), (10.0, -4.0, 6.0)] {
            let (mut c, _) = chain(Levels {
                input_gain_db: gin,
                output_gain_db: gout,
                ..Default::default()
            });
            let out = run(&mut c, &input);
            let change = rms_db(&out[FRAME * 10..]) - rms_db(&input[FRAME * 10..]);
            assert!(
                (change - expect).abs() < 0.05,
                "{gin}/{gout}: {change:.2} dB"
            );
        }
    }

    #[test]
    fn gain_changes_ramp_without_steps() {
        let input = vec![0.1f32; FRAME * 10];
        let (mut c, shared) = chain(Levels::default());
        let mut out = run(&mut c, &input[..FRAME * 5]);
        shared.set(Levels {
            input_gain_db: 12.0,
            ..Default::default()
        });
        out.extend(run(&mut c, &input[FRAME * 5..]));
        let max_step = out
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        // 0.1 -> ~0.4 spread over 480 samples: each step is tiny.
        assert!(max_step < 0.001, "max step {max_step}");
        assert!((out[out.len() - 1] - 0.1 * db_to_gain(12.0)).abs() < 1e-4);
    }

    #[test]
    fn gate_mutes_quiet_noise_and_passes_speech_level() {
        let levels = Levels {
            gate_enabled: true,
            gate_threshold_db: -50.0,
            ..Default::default()
        };
        // -66 dBFS (well below threshold): muted after hold + release.
        let quiet = sine(100, 0.0007, 300.0);
        let (mut c, shared) = chain(levels);
        let out = run(&mut c, &quiet);
        let reduction = rms_db(&quiet[FRAME * 60..]) - rms_db(&out[FRAME * 60..]);
        assert!(reduction > 60.0, "quiet reduced by {reduction:.1} dB");
        assert!(!shared.telemetry().gate_open);
        // -23 dBFS: untouched.
        let loud = sine(50, 0.1, 300.0);
        let (mut c, shared) = chain(levels);
        assert_eq!(run(&mut c, &loud), loud);
        assert!(shared.telemetry().gate_open);
    }

    #[test]
    fn gate_holds_through_short_pauses_and_does_not_chatter() {
        let levels = Levels {
            gate_enabled: true,
            gate_threshold_db: -40.0,
            ..Default::default()
        };
        let (mut c, _) = chain(levels);
        // Speech-level frames with 100 ms pauses (shorter than the 200 ms hold): never closes.
        let mut input = Vec::new();
        for k in 0..20 {
            let amp = if k % 2 == 0 { 0.1 } else { 0.0005 };
            input.extend(sine(10, amp, 200.0));
        }
        let out = run(&mut c, &input);
        assert_eq!(
            out, input,
            "gate closed during a pause shorter than the hold"
        );
        // Level hovering inside the hysteresis band (-41..-43 dB) never closes an open gate.
        let (mut c, _) = chain(levels);
        let mut hover = sine(10, 0.1, 200.0);
        hover.extend(sine(100, 0.0105, 200.0)); // about -42.6 dBFS
        let out = run(&mut c, &hover);
        assert_eq!(out, hover);
    }

    #[test]
    fn gate_opening_and_closing_are_smooth() {
        let levels = Levels {
            gate_enabled: true,
            gate_threshold_db: -40.0,
            ..Default::default()
        };
        let (mut c, _) = chain(levels);
        let mut input = vec![0.0001f32; FRAME * 40]; // closed
        input.extend(vec![0.2f32; FRAME * 10]); // open (DC keeps the check simple)
        input.extend(vec![0.0001f32; FRAME * 40]); // closes after the hold
        let out = run(&mut c, &input);
        // No single-sample jump larger than the input's own step plus the gain glide.
        let open_at = FRAME * 40;
        let step = (out[open_at + 1] - out[open_at]).abs();
        assert!(step < 0.2 * 0.05, "opening jumped by {step}");
        let close_region = &out[FRAME * 50..];
        let max_step = close_region
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_step < 0.2, "closing jumped by {max_step}");
    }

    #[test]
    fn limiter_prevents_clipping_and_reports_it() {
        let input = sine(100, 0.9, 440.0);
        let (mut c, shared) = chain(Levels {
            output_gain_db: 12.0,
            ..Default::default()
        });
        let out = run(&mut c, &input);
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak <= LIMIT + 1e-4, "peak {peak}");
        assert!(shared.telemetry().limited_frames > 90);
        // Below the threshold the limiter is exactly transparent.
        let mut l = Limiter::default();
        let mut f = [0.5f32; FRAME];
        assert_eq!(l.process(&mut f), 0.0);
        assert!(f.iter().all(|&v| v == 0.5));
    }

    #[test]
    fn voice_tracker_rises_gradually_and_decays_slowly() {
        let mut t = VoiceTracker::default();
        // One loud frame barely moves it (10 % of the gap).
        let after_spike = t.update(-10.0);
        assert!(after_spike < -60.0, "{after_spike}");
        // Sustained speech at -20 dBFS is reached within ~0.8 s.
        let mut v = after_spike;
        for _ in 0..80 {
            v = t.update(-20.0);
        }
        assert!((v + 20.0).abs() < 0.05, "{v}");
        // Silence: decays 1 dB per second.
        let before = v;
        for _ in 0..100 {
            v = t.update(-90.0);
        }
        assert!((before - v - 1.0).abs() < 0.01, "decayed {} dB", before - v);
    }

    #[test]
    fn isolation_mutes_a_quieter_talker_in_pauses_and_keeps_the_voice() {
        let levels = Levels {
            isolation_enabled: true,
            isolation_margin_db: 6.0,
            ..Default::default()
        };
        let (mut c, shared) = chain(levels);
        // The user talks (-23 dBFS) for 2 s ...
        let voice = sine(200, 0.1, 220.0);
        let out = run(&mut c, &voice);
        assert_eq!(
            &out[FRAME * 60..],
            &voice[FRAME * 60..],
            "own voice must pass untouched"
        );
        let tracked = shared.telemetry().voice_level_db;
        assert!((tracked + 23.0).abs() < 1.0, "tracked voice {tracked}");
        // ... then pauses while someone 12 dB quieter talks: muted once the 200 ms hold and the
        // ~0.45 s glide to the -80 dB floor are over.
        let other = sine(120, 0.025, 330.0);
        let out = run(&mut c, &other);
        let reduction = rms_db(&other[FRAME * 80..]) - rms_db(&out[FRAME * 80..]);
        assert!(reduction > 60.0, "background reduced by {reduction:.1} dB");
        assert!(!shared.telemetry().gate_open);
        // The user speaks again: passes.
        let again = sine(50, 0.1, 220.0);
        let out = run(&mut c, &again);
        assert_eq!(&out[FRAME * 5..], &again[FRAME * 5..]);
    }

    #[test]
    fn upcoming_speech_opens_the_gate_before_it_arrives() {
        let mut g = Gate::default();
        let quiet = [0.0001f32; FRAME]; // -80 dBFS
        for _ in 0..40 {
            g.process(&mut quiet.clone(), true, -40.0, f32::NEG_INFINITY);
        }
        assert!(!g.is_open());
        // Look-ahead reports speech 6+ dB above the threshold: opens although this frame is quiet.
        g.process(&mut quiet.clone(), true, -40.0, -30.0);
        assert!(g.is_open());
        // Upcoming audio only slightly above the threshold (could be noise) does not open it.
        let mut g = Gate::default();
        for _ in 0..40 {
            g.process(&mut quiet.clone(), true, -40.0, f32::NEG_INFINITY);
        }
        g.process(&mut quiet.clone(), true, -40.0, -37.0);
        assert!(!g.is_open());
    }

    #[test]
    fn isolation_disabled_is_exactly_transparent_even_with_a_delaying_engine() {
        struct Delay3(Vec<f32>);
        impl FrameProcessor for Delay3 {
            fn process(&mut self, frame: &mut [f32; FRAME]) {
                self.0.extend_from_slice(frame);
                frame.copy_from_slice(&self.0[..FRAME]);
                self.0.drain(..FRAME);
            }
            fn delay_frames(&self) -> usize {
                3
            }
        }
        let shared = Arc::new(ChainShared::new(Levels::default()));
        let mut c = ProcessingChain::new(Box::new(Delay3(vec![0.0; FRAME * 3])), shared);
        let input = sine(30, 0.3, 500.0);
        let out = run(&mut c, &input);
        assert_eq!(&out[FRAME * 3..], &input[..input.len() - FRAME * 3]);
    }

    struct DelayEngine(Vec<f32>, usize);
    impl FrameProcessor for DelayEngine {
        fn process(&mut self, frame: &mut [f32; FRAME]) {
            self.0.extend_from_slice(frame);
            frame.copy_from_slice(&self.0[..FRAME]);
            self.0.drain(..FRAME);
        }
        fn delay_frames(&self) -> usize {
            self.1
        }
    }

    #[test]
    fn meters_measure_real_levels_with_ballistics() {
        let (mut c, shared) = chain(Levels {
            output_gain_db: -10.0,
            ..Default::default()
        });
        run(&mut c, &sine(150, 0.1, 440.0)); // 1.5 s of a -23 dBFS RMS / -20 dBFS peak sine
        let m = shared.meters();
        assert!((m.input_rms_db + 23.0).abs() < 0.2, "{m:?}");
        assert!((m.input_peak_db + 20.0).abs() < 0.1, "{m:?}");
        assert!((m.output_rms_db + 33.0).abs() < 0.2, "{m:?}");
        assert!((m.output_peak_db + 30.0).abs() < 0.1, "{m:?}");
        // One second of silence: the peak hold falls ~15 dB, RMS drops much further.
        run(&mut c, &vec![0.0; FRAME * 100]);
        let m = shared.meters();
        assert!((m.input_peak_db + 35.0).abs() < 0.5, "{m:?}");
        assert!(m.input_rms_db < -35.0, "{m:?}");
        // Long silence reaches the floor.
        run(&mut c, &vec![0.0; FRAME * 600]);
        assert_eq!(shared.meters().input_rms_db, METER_FLOOR_DB);
    }

    #[test]
    fn monitor_original_is_aligned_with_processed_and_off_writes_nothing() {
        let input = sine(40, 0.3, 300.0);
        let capture = |source: u8| -> Vec<f32> {
            let shared = Arc::new(ChainShared::new(Levels::default()));
            let (mut link, port) = monitor_link();
            let mut c = ProcessingChain::new(
                Box::new(DelayEngine(vec![0.0; FRAME * 3], 3)),
                shared.clone(),
            )
            .with_monitor(port);
            let (tx, mut rx) = rtrb::RingBuffer::<f32>::new(FRAME * 64);
            link.inbox.push(tx).unwrap();
            shared.monitor_source.store(source, Relaxed);
            run(&mut c, &input);
            let n = rx.slots();
            let chunk = rx.read_chunk(n).unwrap();
            let (a, b) = chunk.as_slices();
            let v: Vec<f32> = a.iter().chain(b).copied().collect();
            chunk.commit_all();
            v
        };
        let original = capture(MONITOR_ORIGINAL);
        let processed = capture(MONITOR_PROCESSED);
        assert_eq!(original.len(), input.len());
        assert_eq!(
            original, processed,
            "original must be delayed exactly like the engine"
        );
        assert_eq!(&processed[FRAME * 3..], &input[..input.len() - FRAME * 3]);
        assert!(capture(MONITOR_OFF).is_empty());
    }

    #[test]
    fn replaced_monitor_rings_are_returned_not_freed_on_the_audio_thread() {
        let shared = Arc::new(ChainShared::new(Levels::default()));
        let (mut link, port) = monitor_link();
        let mut c = ProcessingChain::new(Box::new(Passthrough), shared).with_monitor(port);
        let (first, _keep1) = rtrb::RingBuffer::<f32>::new(FRAME * 4);
        let (second, _keep2) = rtrb::RingBuffer::<f32>::new(FRAME * 4);
        link.inbox.push(first).unwrap();
        run(&mut c, &[0.0; FRAME]);
        assert!(link.returned.pop().is_err());
        link.inbox.push(second).unwrap();
        run(&mut c, &[0.0; FRAME]);
        assert!(link.returned.pop().is_ok(), "the first ring must come back");
    }
}

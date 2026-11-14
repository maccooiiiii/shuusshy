//! Noise suppression engines (spec Part 3.2). Each engine is a [`FrameProcessor`] that runs on the
//! DSP thread; its controls and telemetry are shared lock-free with the controller and UI.
//!
//! [`FrameProcessor`]: crate::audio::pipeline::FrameProcessor

pub mod deepfilter;
pub mod rnnoise;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use serde::Serialize;

/// The selectable engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Rnnoise,
    DeepFilterNet,
}

impl EngineKind {
    pub fn name(self) -> &'static str {
        match self {
            EngineKind::Rnnoise => "RNNoise",
            EngineKind::DeepFilterNet => "DeepFilterNet",
        }
    }
}

/// Live controls (written by the controller, read once per frame by the engine) and telemetry
/// (written by the engine, read by the UI). Changing a control never restarts the pipeline.
#[derive(Debug)]
pub struct EngineShared {
    /// Noise suppression on/off. Off is a genuine bypass: the output is the input, untouched.
    pub enabled: AtomicBool,
    /// 0.0..=1.0, stored as `f32` bits. Meaning is engine-specific (documented per engine).
    pub strength_bits: AtomicU32,
    /// Frames the engine has run.
    pub frames: AtomicU64,
    /// Frames whose engine output was unusable (NaN/inf) and were replaced.
    pub bad_frames: AtomicU64,
    /// Latest voice-activity probability 0..1 (`f32` bits; NaN when the engine has none).
    pub vad_bits: AtomicU32,
    /// Latest local speech-to-noise estimate in dB (`f32` bits; NaN when the engine has none).
    pub snr_bits: AtomicU32,
    /// Exponential average of engine processing time per frame, microseconds (`f32` bits).
    pub avg_us_bits: AtomicU32,
    /// Set by an engine that cannot keep up in real time; it then passes audio through untouched.
    pub overloaded: AtomicBool,
}

/// Time constant of the processing-time average, in frames.
const AVG_FRAMES: f32 = 100.0;

impl EngineShared {
    pub fn new(enabled: bool, strength: f32) -> Self {
        let s = Self {
            enabled: AtomicBool::new(enabled),
            strength_bits: AtomicU32::new(0),
            frames: AtomicU64::new(0),
            bad_frames: AtomicU64::new(0),
            vad_bits: AtomicU32::new(f32::NAN.to_bits()),
            snr_bits: AtomicU32::new(f32::NAN.to_bits()),
            avg_us_bits: AtomicU32::new(0f32.to_bits()),
            overloaded: AtomicBool::new(false),
        };
        s.set(enabled, strength);
        s
    }

    pub fn set(&self, enabled: bool, strength: f32) {
        self.enabled.store(enabled, Relaxed);
        self.strength_bits
            .store(strength.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Relaxed)
    }

    pub fn strength(&self) -> f32 {
        f32::from_bits(self.strength_bits.load(Relaxed))
    }

    pub fn overloaded(&self) -> bool {
        self.overloaded.load(Relaxed)
    }

    /// Clears per-run telemetry when a new pipeline starts (controls are kept).
    pub fn reset_run(&self) {
        self.frames.store(0, Relaxed);
        self.bad_frames.store(0, Relaxed);
        self.vad_bits.store(f32::NAN.to_bits(), Relaxed);
        self.snr_bits.store(f32::NAN.to_bits(), Relaxed);
        self.avg_us_bits.store(0f32.to_bits(), Relaxed);
        self.overloaded.store(false, Relaxed);
    }

    /// Updates the frame counter and the average processing time.
    pub fn record_frame(&self, micros: f32) {
        self.frames.fetch_add(1, Relaxed);
        let avg = f32::from_bits(self.avg_us_bits.load(Relaxed));
        let avg = if avg == 0.0 {
            micros
        } else {
            avg + (micros - avg) / AVG_FRAMES
        };
        self.avg_us_bits.store(avg.to_bits(), Relaxed);
    }

    pub fn telemetry(&self) -> EngineTelemetry {
        let opt = |bits: &AtomicU32| {
            let v = f32::from_bits(bits.load(Relaxed));
            v.is_finite().then_some(v)
        };
        EngineTelemetry {
            frames: self.frames.load(Relaxed),
            bad_frames: self.bad_frames.load(Relaxed),
            vad: opt(&self.vad_bits),
            snr_db: opt(&self.snr_bits),
            avg_ms: f32::from_bits(self.avg_us_bits.load(Relaxed)) / 1000.0,
            overloaded: self.overloaded(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineTelemetry {
    pub frames: u64,
    pub bad_frames: u64,
    /// RNNoise: voice-activity probability 0..1.
    pub vad: Option<f32>,
    /// DeepFilterNet: local speech-to-noise estimate in dB.
    pub snr_db: Option<f32>,
    pub avg_ms: f32,
    pub overloaded: bool,
}

/// Engine state shown to the user (spec Part 4.5). Mirrors `src/lib/status.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EngineStatus {
    /// No audio is flowing through the engine (routing off/waiting, or engine not available).
    Unavailable {
        reason: String,
    },
    Initializing,
    /// Audio is flowing and suppression is applied.
    Processing,
    /// Audio is flowing and suppression is switched off (untouched pass-through).
    Bypassed,
    Interrupted {
        reason: String,
    },
    Error {
        reason: String,
    },
}

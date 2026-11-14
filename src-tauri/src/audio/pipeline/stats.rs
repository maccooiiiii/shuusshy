//! Lock-free counters shared between the audio callbacks, the DSP thread and the UI.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use serde::Serialize;

#[derive(Debug, Default)]
pub struct Stats {
    /// Incremented on every capture / output callback; used as a liveness heartbeat.
    pub capture_callbacks: AtomicU64,
    pub output_callbacks: AtomicU64,
    /// Capture callbacks that had to drop samples because the input ring was full.
    pub overruns: AtomicU64,
    /// Output callbacks that ran out of audio and padded with silence.
    pub underruns: AtomicU64,
    /// Glitch notifications reported by WASAPI (data discontinuities); non-fatal.
    pub xruns: AtomicU64,
    /// Times the output callback discarded excess audio to cap latency.
    pub resyncs: AtomicU64,
    /// 10 ms frames processed by the DSP thread.
    pub frames: AtomicU64,
    /// Total audio buffered inside the pipeline, in microseconds.
    pub buffered_us: AtomicU32,
    /// Current drift correction in ppm, stored as `f32` bits.
    pub drift_ppm_bits: AtomicU32,
    /// Longest time the DSP thread spent processing one frame, in microseconds.
    pub max_frame_us: AtomicU32,
    /// Sum and count of post-pull margins since the drift controller last read them.
    pub margin_sum: AtomicU64,
    pub margin_count: AtomicU32,
    /// Timing peaks since the last `take_peaks` (diagnostics for dropouts).
    pub peaks: PeakCounters,
}

/// Interval maxima/minima, reset by [`Stats::take_peaks`].
#[derive(Debug)]
pub struct PeakCounters {
    /// Largest number of frames the output device asked for in one callback.
    pub out_max_request: AtomicU32,
    /// Largest number of frames the microphone delivered in one callback.
    pub in_max_packet: AtomicU32,
    /// Lowest output-ring level left after an output callback's pull (samples).
    pub out_min_fill: AtomicU32,
    /// Longest gap between consecutive callbacks / DSP loop iterations (microseconds).
    pub out_max_gap_us: AtomicU32,
    pub in_max_gap_us: AtomicU32,
    pub dsp_max_gap_us: AtomicU32,
}

impl Default for PeakCounters {
    fn default() -> Self {
        Self {
            out_max_request: AtomicU32::new(0),
            in_max_packet: AtomicU32::new(0),
            out_min_fill: AtomicU32::new(u32::MAX),
            out_max_gap_us: AtomicU32::new(0),
            in_max_gap_us: AtomicU32::new(0),
            dsp_max_gap_us: AtomicU32::new(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peaks {
    pub out_max_request: u32,
    pub in_max_packet: u32,
    pub out_min_fill: u32,
    pub out_max_gap_ms: f32,
    pub in_max_gap_ms: f32,
    pub dsp_max_gap_ms: f32,
}

/// Microseconds since `last`, updating `last` to now. For use in audio callbacks (QPC only).
pub fn gap_us(last: &mut Option<std::time::Instant>) -> u32 {
    let now = std::time::Instant::now();
    let gap = last.map_or(0, |l| {
        now.duration_since(l).as_micros().min(u32::MAX as u128) as u32
    });
    *last = Some(now);
    gap
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSnapshot {
    pub buffered_ms: f32,
    pub drift_ppm: f32,
    pub underruns: u64,
    pub overruns: u64,
    pub resyncs: u64,
    pub xruns: u64,
    pub frames: u64,
    pub max_frame_ms: f32,
}

impl Stats {
    pub fn set_drift_ppm(&self, ppm: f64) {
        self.drift_ppm_bits.store((ppm as f32).to_bits(), Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            buffered_ms: self.buffered_us.load(Relaxed) as f32 / 1000.0,
            drift_ppm: f32::from_bits(self.drift_ppm_bits.load(Relaxed)),
            underruns: self.underruns.load(Relaxed),
            overruns: self.overruns.load(Relaxed),
            resyncs: self.resyncs.load(Relaxed),
            xruns: self.xruns.load(Relaxed),
            frames: self.frames.load(Relaxed),
            max_frame_ms: self.max_frame_us.load(Relaxed) as f32 / 1000.0,
        }
    }

    /// Returns the timing peaks since the previous call and resets them.
    pub fn take_peaks(&self) -> Peaks {
        let p = &self.peaks;
        let ms = |v: u32| v as f32 / 1000.0;
        Peaks {
            out_max_request: p.out_max_request.swap(0, Relaxed),
            in_max_packet: p.in_max_packet.swap(0, Relaxed),
            out_min_fill: p.out_min_fill.swap(u32::MAX, Relaxed),
            out_max_gap_ms: ms(p.out_max_gap_us.swap(0, Relaxed)),
            in_max_gap_ms: ms(p.in_max_gap_us.swap(0, Relaxed)),
            dsp_max_gap_ms: ms(p.dsp_max_gap_us.swap(0, Relaxed)),
        }
    }
}

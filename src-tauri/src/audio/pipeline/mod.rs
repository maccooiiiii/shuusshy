//! Real-time audio pipeline: microphone -> 48 kHz mono 10 ms frames -> processor -> virtual mic.
//!
//! ```text
//! mic callback --ring_in--> DSP thread --ring_out--> VB-CABLE "CABLE Input" callback
//!  (downmix)               (resample to 48k,          (upmix, prime/underrun/resync)
//!                           frame, process,
//!                           drift-corrected resample)
//! ```

pub mod drift;
mod dsp;
mod io;
pub mod stats;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::thread::JoinHandle;

use cpal::Stream;
use cpal::traits::{DeviceTrait, StreamTrait};
use serde::Serialize;

pub use io::{ErrorSink, StreamFormat};
use io::{OutputBuffering, build_capture, build_output, open_device};
pub use stats::{Peaks, Stats, StatsSnapshot};

/// Internal processing rate. Both RNNoise and DeepFilterNet operate at 48 kHz.
pub const PROCESS_RATE: u32 = 48_000;
/// Samples per processing frame (10 ms at 48 kHz): RNNoise's and DeepFilterNet's hop size.
pub const FRAME: usize = 480;
/// Audio left in the output buffer right after each VB-CABLE pull, maintained by the drift
/// controller. Covers one late microphone packet plus pull jitter (see drift.rs). The average
/// buffer is this plus about half of VB-CABLE's ~20 ms pull.
pub const TARGET_MARGIN_MS: u32 = 20;
/// A post-pull margin above this (after a stall's backlog arrives) is trimmed back to target.
pub const MAX_MARGIN_MS: u32 = 60;
/// Capacity of each ring buffer.
const RING_MS: u32 = 500;

/// Processes one 48 kHz mono frame in place. Runs on the DSP thread: must not block.
pub trait FrameProcessor: Send {
    fn process(&mut self, frame: &mut [f32; FRAME]);

    /// The processor's algorithmic delay in whole frames: output frame `n` carries input frame
    /// `n - delay`. Lets later stages "look ahead" at input that has not come out yet.
    fn delay_frames(&self) -> usize {
        0
    }
}

/// Leaves audio untouched (Milestone 3; noise suppression engines plug in here later).
pub struct Passthrough;

impl FrameProcessor for Passthrough {
    fn process(&mut self, _frame: &mut [f32; FRAME]) {}
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineInfo {
    pub input: StreamFormat,
    pub output: StreamFormat,
    pub target_margin_ms: u32,
}

pub struct Pipeline {
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    dsp: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    pub stats: Arc<Stats>,
    pub info: PipelineInfo,
}

impl Pipeline {
    /// Opens both endpoints by WASAPI endpoint ID and starts audio flowing.
    pub fn start(
        input_id: &str,
        output_id: &str,
        processor: Box<dyn FrameProcessor>,
        on_error: ErrorSink,
    ) -> Result<Self, String> {
        let input_dev = open_device(input_id).map_err(|e| format!("microphone: {e}"))?;
        let output_dev = open_device(output_id).map_err(|e| format!("virtual microphone: {e}"))?;
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));

        let out_rate = output_dev
            .default_output_config()
            .map(|c| c.sample_rate())
            .map_err(|e| format!("cannot read virtual microphone format: {e}"))?;
        let (prod_out, cons_out) = rtrb::RingBuffer::<f32>::new(ring_len(out_rate));
        let (output_stream, output_fmt) = build_output(
            &output_dev,
            cons_out,
            |rate| OutputBuffering {
                target_margin: (rate * TARGET_MARGIN_MS / 1000) as usize,
                max_margin: (rate * MAX_MARGIN_MS / 1000) as usize,
            },
            stats.clone(),
            on_error.clone(),
        )?;

        let in_rate = input_dev
            .default_input_config()
            .map(|c| c.sample_rate())
            .map_err(|e| format!("cannot read microphone format: {e}"))?;
        let (prod_in, cons_in) = rtrb::RingBuffer::<f32>::new(ring_len(in_rate));
        let (input_stream, input_fmt) =
            build_capture(&input_dev, prod_in, stats.clone(), on_error)?;

        // Ring sizes were derived from a separate format query; refuse a mismatch rather than
        // run with wrongly sized buffers (the controller retries).
        if output_fmt.sample_rate != out_rate || input_fmt.sample_rate != in_rate {
            return Err("device format changed during setup; retrying".into());
        }

        let dsp = dsp::spawn(
            dsp::DspSetup {
                input_rate: input_fmt.sample_rate,
                output_rate: output_fmt.sample_rate,
                target_margin: (output_fmt.sample_rate * TARGET_MARGIN_MS / 1000) as usize,
            },
            cons_in,
            prod_out,
            processor,
            stats.clone(),
            stop.clone(),
        )?;

        // From here on, an early return drops `pipeline`, which stops everything started so far.
        let pipeline = Self {
            input_stream: Some(input_stream),
            output_stream: Some(output_stream),
            dsp: Some(dsp),
            stop,
            stats,
            info: PipelineInfo {
                input: input_fmt,
                output: output_fmt,
                target_margin_ms: TARGET_MARGIN_MS,
            },
        };
        if let Some(s) = &pipeline.output_stream {
            s.play()
                .map_err(|e| format!("cannot start virtual microphone: {e}"))?;
        }
        if let Some(s) = &pipeline.input_stream {
            s.play()
                .map_err(|e| format!("cannot start microphone: {e}"))?;
        }
        Ok(pipeline)
    }

    /// Plays 48 kHz mono audio from `source` (e.g. the processing chain's monitor ring) on an
    /// output device, with the same buffering, resampling and clock-drift correction as the main
    /// pipeline. Used for the live test's headphone monitoring.
    pub fn start_playback(
        source: rtrb::Consumer<f32>,
        output_id: &str,
        on_error: ErrorSink,
    ) -> Result<Self, String> {
        let output_dev = open_device(output_id).map_err(|e| format!("monitor device: {e}"))?;
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let out_rate = output_dev
            .default_output_config()
            .map(|c| c.sample_rate())
            .map_err(|e| format!("cannot read monitor device format: {e}"))?;
        let (prod_out, cons_out) = rtrb::RingBuffer::<f32>::new(ring_len(out_rate));
        let (output_stream, output_fmt) = build_output(
            &output_dev,
            cons_out,
            |rate| OutputBuffering {
                target_margin: (rate * TARGET_MARGIN_MS / 1000) as usize,
                max_margin: (rate * MAX_MARGIN_MS / 1000) as usize,
            },
            stats.clone(),
            on_error,
        )?;
        if output_fmt.sample_rate != out_rate {
            return Err("monitor device format changed during setup".into());
        }
        let dsp = dsp::spawn(
            dsp::DspSetup {
                input_rate: PROCESS_RATE,
                output_rate: output_fmt.sample_rate,
                target_margin: (output_fmt.sample_rate * TARGET_MARGIN_MS / 1000) as usize,
            },
            source,
            prod_out,
            Box::new(Passthrough),
            stats.clone(),
            stop.clone(),
        )?;
        let pipeline = Self {
            input_stream: None,
            output_stream: Some(output_stream),
            dsp: Some(dsp),
            stop,
            stats,
            info: PipelineInfo {
                input: StreamFormat {
                    sample_rate: PROCESS_RATE,
                    channels: 1,
                    sample_format: "F32 (Shuusshy monitor)".into(),
                },
                output: output_fmt,
                target_margin_ms: TARGET_MARGIN_MS,
            },
        };
        if let Some(s) = &pipeline.output_stream {
            s.play()
                .map_err(|e| format!("cannot start monitor device: {e}"))?;
        }
        Ok(pipeline)
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        // Stop the producer side first, then the consumer, then the DSP thread.
        drop(self.input_stream.take());
        drop(self.output_stream.take());
        self.stop.store(true, Relaxed);
        if let Some(t) = self.dsp.take() {
            let _ = t.join();
        }
    }
}

fn ring_len(rate: u32) -> usize {
    (rate * RING_MS / 1000) as usize
}

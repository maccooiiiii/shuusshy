//! DSP thread: microphone ring (device rate) -> 48 kHz 10 ms frames -> processor ->
//! drift-compensating resampler -> output ring (virtual cable rate).
//!
//! All buffers are allocated before the loop starts; the loop itself does not allocate.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Adjustable, Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use super::drift::{DriftController, MAX_CORRECTION_PPM};
use super::stats::{Stats, gap_us};
use super::{FRAME, FrameProcessor, PROCESS_RATE};

/// How often the drift controller updates.
const CONTROL_PERIOD: Duration = Duration::from_secs(1);

fn sinc(ratio: f64, max_rel: f64, chunk: usize) -> Result<Async<f32>, String> {
    let params = SincInterpolationParameters::new(64, WindowFunction::BlackmanHarris2)
        .oversampling_factor(128)
        .interpolation(SincInterpolationType::Cubic);
    Async::<f32>::new_sinc(ratio, max_rel, &params, chunk, 1, FixedAsync::Input)
        .map_err(|e| format!("resampler setup failed: {e}"))
}

/// Converts the microphone's native rate to 48 kHz (fixed ratio; no-op at 48 kHz).
struct InputStage {
    resampler: Option<Async<f32>>,
    chunk: usize,
    in_buf: Vec<f32>,
    out_buf: Vec<f32>,
    rate: u32,
}

impl InputStage {
    fn new(rate: u32) -> Result<Self, String> {
        if rate == PROCESS_RATE {
            return Ok(Self {
                resampler: None,
                chunk: 0,
                in_buf: Vec::new(),
                out_buf: Vec::new(),
                rate,
            });
        }
        let chunk = (rate as usize / 100).max(16);
        let r = sinc(PROCESS_RATE as f64 / rate as f64, 1.0, chunk)?;
        let out_max = r.output_frames_max();
        Ok(Self {
            resampler: Some(r),
            chunk,
            in_buf: vec![0.0; chunk],
            out_buf: vec![0.0; out_max],
            rate,
        })
    }

    /// Moves everything available from `ring` into `acc` (at 48 kHz). Never grows `acc` past its
    /// capacity; excess is dropped and reported via the return value.
    fn pull(&mut self, ring: &mut Consumer<f32>, acc: &mut Vec<f32>) -> bool {
        let room = |acc: &Vec<f32>| acc.capacity() - acc.len();
        let mut dropped = false;
        match &mut self.resampler {
            None => {
                let n = ring.slots();
                if let Ok(chunk) = ring.read_chunk(n) {
                    let (a, b) = chunk.as_slices();
                    for part in [a, b] {
                        let take = part.len().min(room(acc));
                        dropped |= take < part.len();
                        acc.extend_from_slice(&part[..take]);
                    }
                    chunk.commit_all();
                }
            }
            Some(r) => {
                while ring.slots() >= self.chunk {
                    if let Ok(chunk) = ring.read_chunk(self.chunk) {
                        let (a, b) = chunk.as_slices();
                        self.in_buf[..a.len()].copy_from_slice(a);
                        self.in_buf[a.len()..].copy_from_slice(b);
                        chunk.commit_all();
                    }
                    let input = InterleavedSlice::new(&self.in_buf[..], 1, self.chunk);
                    let out_len = self.out_buf.len();
                    let output = InterleavedSlice::new_mut(&mut self.out_buf[..], 1, out_len);
                    let produced = match (input, output) {
                        (Ok(i), Ok(mut o)) => r
                            .process_into_buffer(&i, &mut o, None)
                            .map(|(_, n)| n)
                            .unwrap_or(0),
                        _ => 0,
                    };
                    let take = produced.min(room(acc));
                    dropped |= take < produced;
                    acc.extend_from_slice(&self.out_buf[..take]);
                }
            }
        }
        dropped
    }

    fn buffered_seconds(&self, ring: &Consumer<f32>) -> f64 {
        let delay = self.resampler.as_ref().map_or(0, |r| r.output_delay());
        ring.slots() as f64 / self.rate as f64 + delay as f64 / PROCESS_RATE as f64
    }
}

pub struct DspSetup {
    pub input_rate: u32,
    pub output_rate: u32,
    /// Average post-pull margin the drift controller maintains (samples).
    pub target_margin: usize,
}

/// Validates the configuration and starts the DSP thread.
pub fn spawn(
    setup: DspSetup,
    mut ring_in: Consumer<f32>,
    mut ring_out: Producer<f32>,
    mut processor: Box<dyn FrameProcessor>,
    stats: Arc<Stats>,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    let mut input = InputStage::new(setup.input_rate)?;
    let out_ratio = setup.output_rate as f64 / PROCESS_RATE as f64;
    let max_rel = 1.0 + 2.0 * MAX_CORRECTION_PPM / 1e6;
    let mut out_rs = sinc(out_ratio, max_rel, FRAME)?;
    let mut out_buf = vec![0.0f32; out_rs.output_frames_max()];
    // Up to 1 s of 48 kHz audio waiting to be cut into frames.
    let mut acc: Vec<f32> = Vec::with_capacity(PROCESS_RATE as usize);
    let mut frame = [0.0f32; FRAME];
    let mut drift = DriftController::new(setup.target_margin, setup.output_rate);
    let ring_out_capacity = ring_out.buffer().capacity();
    let out_rate = setup.output_rate as f64;

    std::thread::Builder::new()
        .name("shuusshy-dsp".into())
        .spawn(move || {
            let _mmcss = mmcss::ProAudio::enter();
            let mut last_control = Instant::now();
            let mut last_iter = None;
            while !stop.load(Relaxed) {
                stats
                    .peaks
                    .dsp_max_gap_us
                    .fetch_max(gap_us(&mut last_iter), Relaxed);
                if input.pull(&mut ring_in, &mut acc) {
                    stats.overruns.fetch_add(1, Relaxed);
                }
                let mut worked = false;
                while acc.len() >= FRAME {
                    let started = Instant::now();
                    frame.copy_from_slice(&acc[..FRAME]);
                    acc.drain(..FRAME);

                    processor.process(&mut frame);

                    let out_len = out_buf.len();
                    let produced = match (
                        InterleavedSlice::new(&frame[..], 1, FRAME),
                        InterleavedSlice::new_mut(&mut out_buf[..], 1, out_len),
                    ) {
                        (Ok(i), Ok(mut o)) => out_rs
                            .process_into_buffer(&i, &mut o, None)
                            .map(|(_, n)| n)
                            .unwrap_or(0),
                        _ => 0,
                    };
                    let n = produced.min(ring_out.slots());
                    if n < produced {
                        stats.overruns.fetch_add(1, Relaxed);
                    }
                    if let Ok(chunk) = ring_out.write_chunk_uninit(n) {
                        chunk.fill_from_iter(out_buf[..n].iter().copied());
                    }

                    stats.frames.fetch_add(1, Relaxed);
                    let us = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
                    stats.max_frame_us.fetch_max(us, Relaxed);
                    worked = true;
                }
                // Drift control once per second from the average post-pull margin.
                let since = last_control.elapsed();
                if since >= CONTROL_PERIOD {
                    last_control = Instant::now();
                    let count = stats.margin_count.swap(0, Relaxed);
                    let sum = stats.margin_sum.swap(0, Relaxed);
                    if count > 0 {
                        let rel = drift.update(sum as f64 / count as f64, since.as_secs_f64());
                        let _ = out_rs.set_resample_ratio_relative(rel, true);
                        stats.set_drift_ppm(drift.correction_ppm());
                    }
                }
                let out_fill = (ring_out_capacity - ring_out.slots()) as f64 / out_rate;
                let buffered = input.buffered_seconds(&ring_in)
                    + acc.len() as f64 / PROCESS_RATE as f64
                    + out_rs.output_delay() as f64 / out_rate
                    + out_fill;
                stats
                    .buffered_us
                    .store((buffered * 1e6).min(u32::MAX as f64) as u32, Relaxed);
                if !worked {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        })
        .map_err(|e| format!("could not start DSP thread: {e}"))
}

/// macOS: no MMCSS. The DSP thread keeps normal priority; the real-time work happens in
/// CoreAudio's own callbacks, and the ring buffers absorb short scheduling delays.
#[cfg(not(windows))]
mod mmcss {
    pub struct ProAudio;
    impl ProAudio {
        pub fn enter() -> Self {
            Self
        }
    }
}

/// Registers the DSP thread with the Multimedia Class Scheduler ("Pro Audio") for the lifetime of
/// the guard, so Windows schedules it like other real-time audio threads.
#[cfg(windows)]
mod mmcss {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::{
        AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
    };
    use windows::core::w;

    pub struct ProAudio(Option<HANDLE>);

    impl ProAudio {
        pub fn enter() -> Self {
            let mut index = 0u32;
            match unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut index) } {
                Ok(h) => Self(Some(h)),
                Err(e) => {
                    eprintln!("shuusshy: MMCSS registration failed ({e}); using normal priority");
                    Self(None)
                }
            }
        }
    }

    impl Drop for ProAudio {
        fn drop(&mut self) {
            if let Some(h) = self.0.take() {
                let _ = unsafe { AvRevertMmThreadCharacteristics(h) };
            }
        }
    }
}

//! cpal stream construction. The data callbacks here run on high-priority audio threads, so they
//! never block, lock, allocate or print: they only move samples through lock-free ring buffers
//! and bump atomic counters.

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, DeviceId, FromSample, HostId, SampleFormat, SizedSample, Stream};
use rtrb::{Consumer, Producer};
use serde::Serialize;

use super::stats::{Stats, gap_us};

/// Called (off the audio thread's hot path) when a stream reports an error.
pub type ErrorSink = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

pub fn open_device(endpoint_id: &str) -> Result<Device, String> {
    let host =
        cpal::host_from_id(HostId::Wasapi).map_err(|e| format!("WASAPI unavailable: {e}"))?;
    host.device_by_id(&DeviceId::new(HostId::Wasapi, endpoint_id))
        .ok_or_else(|| "device not found (disconnected?)".to_string())
}

fn describe_stream_error(e: &cpal::Error) -> String {
    match e.kind() {
        cpal::ErrorKind::DeviceNotAvailable => "device was disconnected".into(),
        _ => e.to_string(),
    }
}

/// Error callback shared by both streams. Glitch notifications (`Xrun`, e.g. WASAPI's
/// data-discontinuity flag) are counted, not treated as failures; everything else is reported.
fn error_handler(
    side: &'static str,
    stats: Arc<Stats>,
    on_error: ErrorSink,
) -> impl FnMut(cpal::Error) + Send + 'static {
    move |e| {
        if e.kind() == cpal::ErrorKind::Xrun {
            stats.xruns.fetch_add(1, Relaxed);
        } else {
            on_error(format!("{side}: {}", describe_stream_error(&e)));
        }
    }
}

/// Opens the microphone in shared mode at its native format and pushes a mono downmix
/// (average of all channels) into `producer`.
pub fn build_capture(
    device: &Device,
    producer: Producer<f32>,
    stats: Arc<Stats>,
    on_error: ErrorSink,
) -> Result<(Stream, StreamFormat), String> {
    let supported = device
        .default_input_config()
        .map_err(|e| format!("cannot read microphone format: {e}"))?;
    let format = StreamFormat {
        sample_rate: supported.sample_rate(),
        channels: supported.channels(),
        sample_format: format!("{:?}", supported.sample_format()),
    };
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => capture_typed::<f32>(device, config, producer, stats, on_error),
        SampleFormat::I16 => capture_typed::<i16>(device, config, producer, stats, on_error),
        SampleFormat::I32 => capture_typed::<i32>(device, config, producer, stats, on_error),
        SampleFormat::U16 => capture_typed::<u16>(device, config, producer, stats, on_error),
        other => return Err(format!("unsupported microphone sample format {other:?}")),
    }
    .map_err(|e| format!("cannot open microphone: {e}"))?;
    Ok((stream, format))
}

fn capture_typed<T>(
    device: &Device,
    config: cpal::StreamConfig,
    mut producer: Producer<f32>,
    stats: Arc<Stats>,
    on_error: ErrorSink,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let inv = 1.0 / channels as f32;
    let on_err = error_handler("microphone", stats.clone(), on_error);
    let mut last = None;
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let frames = data.len() / channels;
            stats
                .peaks
                .in_max_gap_us
                .fetch_max(gap_us(&mut last), Relaxed);
            stats.peaks.in_max_packet.fetch_max(frames as u32, Relaxed);
            let n = frames.min(producer.slots());
            if n < frames {
                stats.overruns.fetch_add(1, Relaxed);
            }
            if let Ok(chunk) = producer.write_chunk_uninit(n) {
                chunk.fill_from_iter(
                    data.chunks_exact(channels)
                        .map(|f| f.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() * inv),
                );
            }
            stats.capture_callbacks.fetch_add(1, Relaxed);
        },
        on_err,
        None,
    )
}

/// Output-side buffering policy, in output-rate samples. Both values refer to the *margin*: the
/// audio left in the buffer right after the device's pull (see `drift.rs`).
#[derive(Debug, Clone, Copy)]
pub struct OutputBuffering {
    /// Playback (re)starts once a pull would leave this margin; resyncs trim back to it; the
    /// drift controller steers the average margin to it.
    pub target_margin: usize,
    /// A pull that would leave more than this triggers a resync (caps latency after a stall).
    pub max_margin: usize,
}

/// Opens the virtual cable's playback endpoint and plays mono audio from `consumer` on all
/// channels. Pads with silence (and re-primes) on underrun.
pub fn build_output(
    device: &Device,
    consumer: Consumer<f32>,
    buffering_for: impl FnOnce(u32) -> OutputBuffering,
    stats: Arc<Stats>,
    on_error: ErrorSink,
) -> Result<(Stream, StreamFormat), String> {
    let supported = device
        .default_output_config()
        .map_err(|e| format!("cannot read virtual microphone format: {e}"))?;
    let format = StreamFormat {
        sample_rate: supported.sample_rate(),
        channels: supported.channels(),
        sample_format: format!("{:?}", supported.sample_format()),
    };
    let buffering = buffering_for(format.sample_rate);
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => {
            output_typed::<f32>(device, config, consumer, buffering, stats, on_error)
        }
        SampleFormat::I16 => {
            output_typed::<i16>(device, config, consumer, buffering, stats, on_error)
        }
        SampleFormat::I32 => {
            output_typed::<i32>(device, config, consumer, buffering, stats, on_error)
        }
        SampleFormat::U16 => {
            output_typed::<u16>(device, config, consumer, buffering, stats, on_error)
        }
        other => {
            return Err(format!(
                "unsupported virtual microphone sample format {other:?}"
            ));
        }
    }
    .map_err(|e| {
        // AUDCLNT_E_DEVICE_IN_USE (0x8889000A): VB-CABLE's "CABLE Input" and "CABLE In 16ch"
        // share one cable, so another app playing into either one locks Shuusshy out.
        if e.kind() == cpal::ErrorKind::DeviceBusy || e.to_string().contains("-2004287478") {
            "the virtual cable is busy: another app is playing into \"CABLE Input\" or \"CABLE In 16ch\" (check that no app uses a CABLE device as its speaker/output)".to_string()
        } else {
            format!("cannot open virtual microphone: {e}")
        }
    })?;
    Ok((stream, format))
}

fn output_typed<T>(
    device: &Device,
    config: cpal::StreamConfig,
    mut consumer: Consumer<f32>,
    buffering: OutputBuffering,
    stats: Arc<Stats>,
    on_error: ErrorSink,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut priming = true;
    let on_err = error_handler("virtual microphone", stats.clone(), on_error);
    let mut last = None;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            stats.output_callbacks.fetch_add(1, Relaxed);
            let frames = data.len() / channels;
            let mut available = consumer.slots();
            stats
                .peaks
                .out_max_gap_us
                .fetch_max(gap_us(&mut last), Relaxed);
            stats
                .peaks
                .out_max_request
                .fetch_max(frames as u32, Relaxed);

            if available > frames + buffering.max_margin {
                let skip = available - frames - buffering.target_margin;
                if let Ok(chunk) = consumer.read_chunk(skip) {
                    chunk.commit_all();
                }
                stats.resyncs.fetch_add(1, Relaxed);
                available -= skip;
            }
            if priming {
                if available >= frames + buffering.target_margin {
                    priming = false;
                } else {
                    data.fill(T::EQUILIBRIUM);
                    return;
                }
            }

            // Margin left after this pull: the drift controller's input (averaged per second).
            let margin = available.saturating_sub(frames);
            stats.margin_sum.fetch_add(margin as u64, Relaxed);
            stats.margin_count.fetch_add(1, Relaxed);
            stats.peaks.out_min_fill.fetch_min(margin as u32, Relaxed);

            let n = frames.min(available);
            if let Ok(chunk) = consumer.read_chunk(n) {
                let (a, b) = chunk.as_slices();
                for (frame, &s) in data.chunks_exact_mut(channels).zip(a.iter().chain(b)) {
                    frame.fill(T::from_sample(s));
                }
                chunk.commit_all();
            }
            if n < frames {
                data[n * channels..].fill(T::EQUILIBRIUM);
                stats.underruns.fetch_add(1, Relaxed);
                priming = true;
            }
        },
        on_err,
        None,
    )
}

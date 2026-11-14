//! Audio controller: owns the running pipeline on a dedicated thread and keeps it in line with
//! what *should* be running (selected mic + VB-CABLE + routing enabled).
//!
//! It reconciles on request (device change, selection or setting change), restarts after stream
//! errors or stalls, and retries failed starts with backoff. It never reports `running` unless a
//! pipeline actually started and its audio callbacks keep arriving.

use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::chain::{
    ChainShared, ChainTelemetry, Levels, MONITOR_OFF, MONITOR_ORIGINAL, MONITOR_PROCESSED, Meters,
    MonitorLink, ProcessingChain, monitor_link,
};
use super::engines::{
    EngineKind, EngineShared, EngineStatus, EngineTelemetry, deepfilter::DeepFilterProcessor,
    rnnoise::RnnoiseProcessor,
};
use super::pipeline::FrameProcessor;
use super::pipeline::{Pipeline, PipelineInfo, Stats, StatsSnapshot};

/// Noise-suppression settings applied live to the running engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Processing {
    /// `Err(reason)` when the selected engine cannot run in this build.
    pub engine: Result<EngineKind, String>,
    pub enabled: bool,
    pub strength: f32,
    /// Gains and noise gate around the engine (always live).
    pub levels: Levels,
}

/// Endpoints the pipeline should connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub input_id: String,
    pub input_name: String,
    pub output_id: String,
    pub output_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desired {
    Run(Target),
    /// The user turned routing off.
    Off,
    /// Routing is wanted but cannot run yet (e.g. mic disconnected, VB-CABLE missing).
    Waiting(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum RoutingState {
    Off,
    Waiting {
        reason: String,
    },
    Starting,
    Running,
    /// Was running, then failed; a restart is scheduled.
    Interrupted {
        reason: String,
    },
    /// Could not start; a retry is scheduled.
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingStatus {
    #[serde(flatten)]
    pub state: RoutingState,
    pub input: Option<String>,
    pub output: Option<String>,
    pub info: Option<PipelineInfo>,
    pub stats: Option<StatsSnapshot>,
    pub uptime_s: Option<f64>,
    /// Noise-suppression engine state, derived from routing state and engine settings.
    pub engine: EngineStatus,
    pub engine_name: Option<&'static str>,
    pub engine_telemetry: Option<EngineTelemetry>,
    /// Noise gate state and limiter activity while running.
    pub chain_telemetry: Option<ChainTelemetry>,
    /// Live test monitoring, while active.
    pub monitor: Option<MonitorStatus>,
    /// Why monitoring stopped or could not start (cleared when it starts again).
    pub monitor_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatus {
    pub device: String,
    /// "original" or "processed".
    pub source: &'static str,
    pub dropouts: u64,
}

/// What the live test plays to the monitor device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSource {
    Original,
    Processed,
}

impl MonitorSource {
    fn code(self) -> u8 {
        match self {
            MonitorSource::Original => MONITOR_ORIGINAL,
            MonitorSource::Processed => MONITOR_PROCESSED,
        }
    }
    fn name(self) -> &'static str {
        match self {
            MonitorSource::Original => "original",
            MonitorSource::Processed => "processed",
        }
    }
}

enum Ctl {
    Reconcile,
    /// Engine settings changed; publish the new status.
    Notify,
    StreamError {
        generation: u64,
        message: String,
    },
    MonitorStart {
        device_id: String,
        device_name: String,
        source: MonitorSource,
    },
    MonitorSource(MonitorSource),
    MonitorStop,
    MonitorError {
        session: u64,
        message: String,
    },
    Shutdown,
}

struct Shared {
    state: RoutingState,
    target: Option<Target>,
    info: Option<PipelineInfo>,
    stats: Option<Arc<Stats>>,
    started: Option<Instant>,
    engine: Arc<EngineShared>,
    chain: Arc<ChainShared>,
    engine_ok: Result<EngineKind, String>,
    monitor: Option<(String, MonitorSource, Arc<Stats>)>,
    monitor_error: Option<String>,
}

pub type DesiredFn = Box<dyn Fn() -> Desired + Send>;
pub type StateFn = Box<dyn Fn(&RoutingStatus) + Send>;

pub struct AudioController {
    tx: Sender<Ctl>,
    shared: Arc<Mutex<Shared>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

const TICK: Duration = Duration::from_secs(1);
/// Consecutive ticks without callbacks before a running pipeline is considered stalled.
const STALL_TICKS: u32 = 2;
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);

impl AudioController {
    pub fn start(desired: DesiredFn, on_state: StateFn, processing: Processing) -> Self {
        let (tx, rx) = mpsc::channel();
        let engine = Arc::new(EngineShared::new(
            processing.enabled && processing.engine.is_ok(),
            processing.strength,
        ));
        let shared = Arc::new(Mutex::new(Shared {
            state: RoutingState::Waiting {
                reason: "Starting audio".into(),
            },
            target: None,
            info: None,
            stats: None,
            started: None,
            engine,
            chain: Arc::new(ChainShared::new(processing.levels)),
            engine_ok: processing.engine,
            monitor: None,
            monitor_error: None,
        }));
        let worker = Worker {
            tx: tx.clone(),
            shared: shared.clone(),
            desired,
            on_state,
            pipeline: None,
            generation: 0,
            retry_at: None,
            retry_delay: RETRY_MIN,
            last_beats: (0, 0),
            stalled_ticks: 0,
            monitor_link: None,
            monitor: None,
            monitor_session: 0,
        };
        let thread = std::thread::Builder::new()
            .name("shuusshy-audio".into())
            .spawn(move || worker.run(rx))
            .expect("spawn audio controller thread");
        let _ = tx.send(Ctl::Reconcile);
        Self {
            tx,
            shared,
            thread: Mutex::new(Some(thread)),
        }
    }

    /// Asks the controller to re-evaluate what should be running (non-blocking).
    pub fn reconcile(&self) {
        let _ = self.tx.send(Ctl::Reconcile);
    }

    /// Applies noise-suppression settings. On/off and strength take effect on the running
    /// engine immediately; a different engine restarts the pipeline with it.
    pub fn set_processing(&self, p: Processing) {
        let engine_changed = {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            s.engine.set(p.enabled && p.engine.is_ok(), p.strength);
            s.chain.set(p.levels);
            let changed = s.engine_ok != p.engine;
            s.engine_ok = p.engine;
            changed
        };
        let _ = self.tx.send(if engine_changed {
            Ctl::Reconcile
        } else {
            Ctl::Notify
        });
    }

    /// Starts monitoring `source` on an output device (stops any previous monitoring).
    pub fn monitor_start(&self, device_id: String, device_name: String, source: MonitorSource) {
        let _ = self.tx.send(Ctl::MonitorStart {
            device_id,
            device_name,
            source,
        });
    }

    /// Switches what is monitored, without restarting the monitor stream.
    pub fn monitor_source(&self, source: MonitorSource) {
        let _ = self.tx.send(Ctl::MonitorSource(source));
    }

    /// Stops monitoring. The monitored signal is muted immediately, before the stream closes.
    pub fn monitor_stop(&self) {
        if let Ok(s) = self.shared.lock() {
            s.chain.monitor_source.store(MONITOR_OFF, Relaxed);
        }
        let _ = self.tx.send(Ctl::MonitorStop);
    }

    /// Live meter levels while audio is flowing.
    pub fn meters(&self) -> Option<Meters> {
        let s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        matches!(s.state, RoutingState::Running).then(|| s.chain.meters())
    }

    pub fn status(&self) -> RoutingStatus {
        status_of(&self.shared.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Stops the pipeline (so VB-CABLE goes silent cleanly) and joins the thread.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Ctl::Shutdown);
        if let Some(t) = self.thread.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
    }
}

fn engine_status(s: &Shared) -> EngineStatus {
    match &s.state {
        RoutingState::Running => match &s.engine_ok {
            Err(reason) => EngineStatus::Unavailable {
                reason: reason.clone(),
            },
            Ok(kind) if s.engine.overloaded() => EngineStatus::Error {
                reason: format!(
                    "{} is too slow on this PC — audio passes through unprocessed; choose RNNoise",
                    kind.name()
                ),
            },
            Ok(_) if s.engine.enabled() => EngineStatus::Processing,
            Ok(_) => EngineStatus::Bypassed,
        },
        RoutingState::Starting => EngineStatus::Initializing,
        RoutingState::Off => EngineStatus::Unavailable {
            reason: "Virtual mic is off — no audio is being processed".into(),
        },
        RoutingState::Waiting { reason } => EngineStatus::Unavailable {
            reason: reason.clone(),
        },
        RoutingState::Interrupted { reason } => EngineStatus::Interrupted {
            reason: reason.clone(),
        },
        RoutingState::Error { message } => EngineStatus::Error {
            reason: message.clone(),
        },
    }
}

fn status_of(s: &Shared) -> RoutingStatus {
    let running = matches!(s.state, RoutingState::Running);
    RoutingStatus {
        engine: engine_status(s),
        engine_name: s.engine_ok.as_ref().ok().map(|k| k.name()),
        engine_telemetry: running.then(|| s.engine.telemetry()),
        chain_telemetry: running.then(|| s.chain.telemetry()),
        monitor: s
            .monitor
            .as_ref()
            .map(|(device, source, stats)| MonitorStatus {
                device: device.clone(),
                source: source.name(),
                dropouts: stats.underruns.load(Relaxed),
            }),
        monitor_error: s.monitor_error.clone(),
        state: s.state.clone(),
        input: s.target.as_ref().map(|t| t.input_name.clone()),
        output: s.target.as_ref().map(|t| t.output_name.clone()),
        info: if running { s.info.clone() } else { None },
        stats: if running {
            s.stats.as_ref().map(|st| st.snapshot())
        } else {
            None
        },
        uptime_s: s
            .started
            .filter(|_| running)
            .map(|t| t.elapsed().as_secs_f64()),
    }
}

struct Worker {
    tx: Sender<Ctl>,
    shared: Arc<Mutex<Shared>>,
    desired: DesiredFn,
    on_state: StateFn,
    pipeline: Option<(Pipeline, Target, EngineKind)>,
    generation: u64,
    retry_at: Option<Instant>,
    retry_delay: Duration,
    last_beats: (u64, u64),
    stalled_ticks: u32,
    /// Hand-over link to the running chain's monitor port.
    monitor_link: Option<MonitorLink>,
    /// The headphone monitoring playback, while the live test is on.
    monitor: Option<Pipeline>,
    monitor_session: u64,
}

impl Worker {
    fn run(mut self, rx: mpsc::Receiver<Ctl>) {
        loop {
            match rx.recv_timeout(TICK) {
                Ok(Ctl::Reconcile) => self.reconcile(),
                Ok(Ctl::Notify) => {
                    let status = status_of(&self.shared.lock().unwrap_or_else(|p| p.into_inner()));
                    (self.on_state)(&status);
                }
                Ok(Ctl::StreamError {
                    generation,
                    message,
                }) => {
                    if generation == self.generation && self.pipeline.is_some() {
                        eprintln!("shuusshy: audio stream error: {message}");
                        self.fail(RoutingState::Interrupted { reason: message });
                    }
                }
                Ok(Ctl::MonitorStart {
                    device_id,
                    device_name,
                    source,
                }) => self.start_monitor(&device_id, device_name, source),
                Ok(Ctl::MonitorSource(source)) => {
                    let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
                    if s.monitor.is_some() {
                        s.chain.monitor_source.store(source.code(), Relaxed);
                        if let Some(m) = &mut s.monitor {
                            m.1 = source;
                        }
                    }
                    drop(s);
                    self.notify();
                }
                Ok(Ctl::MonitorStop) => {
                    self.stop_monitor(None);
                    self.notify();
                }
                Ok(Ctl::MonitorError { session, message }) => {
                    if session == self.monitor_session && self.monitor.is_some() {
                        self.stop_monitor(Some(format!("Monitoring stopped: {message}")));
                        self.notify();
                    }
                }
                Ok(Ctl::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => self.tick(),
            }
        }
        self.drop_pipeline();
    }

    fn notify(&self) {
        let status = status_of(&self.shared.lock().unwrap_or_else(|p| p.into_inner()));
        (self.on_state)(&status);
    }

    /// Drops the running pipeline, stopping any monitoring that depends on it first.
    fn drop_pipeline(&mut self) {
        if self.monitor.is_some() {
            self.stop_monitor(Some("Monitoring stopped because audio restarted".into()));
        }
        self.pipeline = None;
        self.monitor_link = None;
    }

    fn stop_monitor(&mut self, error: Option<String>) {
        let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        s.chain.monitor_source.store(MONITOR_OFF, Relaxed);
        s.monitor = None;
        s.monitor_error = error;
        drop(s);
        self.monitor = None;
    }

    fn start_monitor(&mut self, device_id: &str, device_name: String, source: MonitorSource) {
        self.stop_monitor(None);
        let Some(link) = &mut self.monitor_link else {
            self.shared
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .monitor_error =
                Some("Audio is not running, so there is nothing to monitor".into());
            self.notify();
            return;
        };
        // 500 ms of 48 kHz mono between the chain and the monitor playback.
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(24_000);
        if link.inbox.push(producer).is_err() {
            self.shared
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .monitor_error = Some("Monitor is busy, try again".into());
            self.notify();
            return;
        }
        self.monitor_session += 1;
        let session = self.monitor_session;
        let tx = self.tx.clone();
        let on_error = Arc::new(move |message: String| {
            let _ = tx.send(Ctl::MonitorError { session, message });
        });
        match Pipeline::start_playback(consumer, device_id, on_error) {
            Ok(p) => {
                eprintln!(
                    "shuusshy: monitoring {} on {device_name} ({:?})",
                    source.name(),
                    p.info.output
                );
                let stats = p.stats.clone();
                self.monitor = Some(p);
                let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
                s.chain.monitor_source.store(source.code(), Relaxed);
                s.monitor = Some((device_name, source, stats));
                s.monitor_error = None;
            }
            Err(e) => {
                eprintln!("shuusshy: could not start monitoring: {e}");
                self.shared
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .monitor_error = Some(format!("Could not start monitoring: {e}"));
            }
        }
        self.notify();
    }

    fn set(&self, state: RoutingState, update: impl FnOnce(&mut Shared)) {
        let status = {
            let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            let changed = s.state != state;
            s.state = state;
            update(&mut s);
            if !changed {
                return;
            }
            status_of(&s)
        };
        (self.on_state)(&status);
    }

    /// Stops the pipeline and schedules a retry with backoff.
    fn fail(&mut self, state: RoutingState) {
        self.drop_pipeline();
        self.retry_at = Some(Instant::now() + self.retry_delay);
        self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
        self.set(state, |s| {
            s.stats = None;
            s.info = None;
            s.started = None;
        });
    }

    fn tick(&mut self) {
        // Free monitor rings the audio thread handed back.
        if let Some(link) = &mut self.monitor_link {
            while link.returned.pop().is_ok() {}
        }
        if let Some((p, _, _)) = &self.pipeline {
            let beats = (
                p.stats.capture_callbacks.load(Relaxed),
                p.stats.output_callbacks.load(Relaxed),
            );
            let mic_stalled = beats.0 == self.last_beats.0;
            let out_stalled = beats.1 == self.last_beats.1;
            self.last_beats = beats;
            self.stalled_ticks = if mic_stalled || out_stalled {
                self.stalled_ticks + 1
            } else {
                0
            };
            if self.stalled_ticks >= STALL_TICKS {
                let which = if mic_stalled {
                    "microphone"
                } else {
                    "virtual microphone"
                };
                eprintln!("shuusshy: audio stalled ({which}); restarting");
                self.fail(RoutingState::Interrupted {
                    reason: format!("{which} stopped delivering audio"),
                });
            }
        } else if self.retry_at.is_some_and(|t| Instant::now() >= t) {
            self.retry_at = None;
            self.reconcile();
        }
    }

    fn reconcile(&mut self) {
        match (self.desired)() {
            Desired::Off => {
                self.drop_pipeline();
                self.retry_at = None;
                self.set(RoutingState::Off, |s| {
                    s.target = None;
                    s.stats = None;
                    s.info = None;
                    s.started = None;
                });
            }
            Desired::Waiting(reason) => {
                self.drop_pipeline();
                self.retry_at = None;
                self.set(RoutingState::Waiting { reason }, |s| {
                    s.target = None;
                    s.stats = None;
                    s.info = None;
                    s.started = None;
                });
            }
            Desired::Run(target) => {
                let (engine, chain, kind) = {
                    let s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
                    // An unavailable engine falls back to RNNoise running bypassed (the status
                    // reports the engine as unavailable), so audio still reaches the virtual mic.
                    (
                        s.engine.clone(),
                        s.chain.clone(),
                        s.engine_ok.clone().unwrap_or(EngineKind::Rnnoise),
                    )
                };
                if self
                    .pipeline
                    .as_ref()
                    .is_some_and(|(_, t, k)| *t == target && *k == kind)
                {
                    return; // already running the right thing
                }
                self.drop_pipeline();
                self.generation += 1;
                let generation = self.generation;
                let t = target.clone();
                self.set(RoutingState::Starting, |s| s.target = Some(t));

                let tx = self.tx.clone();
                let on_error = Arc::new(move |message: String| {
                    let _ = tx.send(Ctl::StreamError {
                        generation,
                        message,
                    });
                });
                // Allocates the engine here, off the audio threads.
                engine.reset_run();
                chain.reset_run();
                let engine_processor: Box<dyn FrameProcessor> = match kind {
                    EngineKind::Rnnoise => Box::new(RnnoiseProcessor::new(engine)),
                    EngineKind::DeepFilterNet => Box::new(DeepFilterProcessor::new(engine)),
                };
                let (link, port) = monitor_link();
                let processor =
                    Box::new(ProcessingChain::new(engine_processor, chain).with_monitor(port));
                match Pipeline::start(&target.input_id, &target.output_id, processor, on_error) {
                    Ok(p) => {
                        eprintln!(
                            "shuusshy: routing {} -> {} with {} ({:?})",
                            target.input_name,
                            target.output_name,
                            kind.name(),
                            p.info
                        );
                        let (stats, info) = (p.stats.clone(), p.info.clone());
                        self.pipeline = Some((p, target, kind));
                        self.monitor_link = Some(link);
                        self.retry_at = None;
                        self.retry_delay = RETRY_MIN;
                        self.last_beats = (0, 0);
                        self.stalled_ticks = 0;
                        self.set(RoutingState::Running, |s| {
                            s.stats = Some(stats);
                            s.info = Some(info);
                            s.started = Some(Instant::now());
                        });
                    }
                    Err(message) => {
                        eprintln!("shuusshy: could not start routing: {message}");
                        self.fail(RoutingState::Error { message });
                    }
                }
            }
        }
    }
}

//! Device monitor for macOS: re-enumerates CoreAudio devices every [`POLL`] on its own thread
//! and publishes a new snapshot only when something changed. Same interface as the Windows
//! monitor (which uses endpoint notifications instead).

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use super::Snapshot;
use super::coreaudio;

/// How often devices are re-listed (plugging in a headset shows up within this time).
const POLL: Duration = Duration::from_millis(1500);

enum Msg {
    Refresh(Sender<()>),
    Shutdown,
}

/// Callback invoked on the monitor thread with each changed snapshot.
pub type OnChange = Box<dyn Fn(&Snapshot) + Send + 'static>;

pub struct DeviceMonitor {
    tx: Sender<Msg>,
    snapshot: Arc<RwLock<Arc<Snapshot>>>,
    thread: Option<JoinHandle<()>>,
}

impl DeviceMonitor {
    /// Starts the monitor thread after a first enumeration on the calling thread.
    pub fn start(on_change: OnChange) -> Result<Self, String> {
        let first = coreaudio::snapshot();
        if let Some(e) = &first.error
            && first.inputs.is_empty()
            && first.outputs.is_empty()
        {
            return Err(e.clone());
        }
        eprintln!(
            "shuusshy: device manager started: {} inputs, {} outputs (polling)",
            first.inputs.len(),
            first.outputs.len()
        );
        let snapshot = Arc::new(RwLock::new(Arc::new(first)));
        let shared = snapshot.clone();
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("shuusshy-devices".into())
            .spawn(move || run(rx, shared, on_change))
            .map_err(|e| format!("could not start device thread: {e}"))?;
        Ok(Self {
            tx,
            snapshot,
            thread: Some(thread),
        })
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot
            .read()
            .map(|s| s.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    /// Re-enumerates now and waits (bounded) for it to finish.
    pub fn refresh(&self) -> Arc<Snapshot> {
        let (done_tx, done_rx) = mpsc::channel();
        if self.tx.send(Msg::Refresh(done_tx)).is_ok() {
            let _ = done_rx.recv_timeout(Duration::from_secs(5));
        }
        self.snapshot()
    }
}

impl Drop for DeviceMonitor {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(rx: Receiver<Msg>, shared: Arc<RwLock<Arc<Snapshot>>>, on_change: OnChange) {
    let publish = |snap: Snapshot| {
        let changed = **shared.read().unwrap_or_else(|p| p.into_inner()) != snap;
        if changed {
            *shared.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(snap.clone());
            on_change(&snap);
        }
    };
    loop {
        match rx.recv_timeout(POLL) {
            Err(RecvTimeoutError::Timeout) => publish(coreaudio::snapshot()),
            Ok(Msg::Refresh(done)) => {
                publish(coreaudio::snapshot());
                let _ = done.send(());
            }
            Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

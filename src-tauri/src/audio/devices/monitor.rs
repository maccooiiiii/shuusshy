//! Device monitor thread: owns the COM objects, listens for Windows endpoint notifications,
//! debounces them, re-enumerates, and publishes a new snapshot only when something changed.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMNotificationClient, IMMNotificationClient_Impl,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::{PCWSTR, implement};

use super::Snapshot;
use super::wasapi::{Wasapi, describe_error};

/// Notifications often arrive in bursts (one device change fires several callbacks).
const DEBOUNCE: Duration = Duration::from_millis(250);
/// How often to check which apps are playing into the virtual cable.
const SESSION_POLL: Duration = Duration::from_secs(2);
/// Upper bound on how long a continuous stream of notifications can delay a refresh.
const MAX_DEBOUNCE: Duration = Duration::from_secs(1);

enum Msg {
    Changed,
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
    /// Starts the monitor thread and waits for the first enumeration.
    pub fn start(on_change: OnChange) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let snapshot = Arc::new(RwLock::new(Arc::new(Snapshot::default())));
        let shared = snapshot.clone();
        let notify_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("shuusshy-devices".into())
            .spawn(move || run(rx, notify_tx, shared, on_change, ready_tx))
            .map_err(|e| format!("could not start device thread: {e}"))?;
        ready_rx
            .recv()
            .map_err(|_| "device thread exited during startup".to_string())??;
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

fn run(
    rx: Receiver<Msg>,
    notify_tx: Sender<Msg>,
    shared: Arc<RwLock<Arc<Snapshot>>>,
    on_change: OnChange,
    ready: Sender<Result<(), String>>,
) {
    // SAFETY: this thread is dedicated to COM work and uninitialises on exit.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        let _ = ready.send(Err(format!("COM init failed: {}", describe_error(&e))));
        return;
    }
    let result = (|| {
        let api = Wasapi::new()
            .map_err(|e| format!("audio device API unavailable: {}", describe_error(&e)))?;
        let client: IMMNotificationClient = Notifier(notify_tx).into();
        let registered = unsafe {
            api.enumerator()
                .RegisterEndpointNotificationCallback(&client)
        };
        if let Err(e) = &registered {
            eprintln!(
                "shuusshy: device notifications unavailable ({}); hot-plug will need manual refresh",
                describe_error(e)
            );
        }

        let publish = |snap: Snapshot| {
            let changed = **shared.read().unwrap_or_else(|p| p.into_inner()) != snap;
            if changed {
                *shared.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(snap.clone());
                on_change(&snap);
            }
        };

        let first = api.snapshot();
        eprintln!(
            "shuusshy: device manager started: {} inputs, {} outputs, notifications={}",
            first.inputs.len(),
            first.outputs.len(),
            if registered.is_ok() { "on" } else { "off" }
        );
        *shared.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(first);
        let _ = ready.send(Ok(()));

        loop {
            match rx.recv_timeout(SESSION_POLL) {
                // Audio sessions change without device notifications, so poll which apps are
                // playing into the virtual cable (cheap: no device is opened).
                Err(RecvTimeoutError::Timeout) => {
                    let current = shared.read().unwrap_or_else(|p| p.into_inner()).clone();
                    let users = api.cable_users(&current.outputs);
                    if users != current.cable_users {
                        let mut next = (*current).clone();
                        next.cable_users = users;
                        publish(next);
                    }
                }
                Ok(Msg::Changed) => {
                    // Debounce: wait until notifications pause for DEBOUNCE, but never longer
                    // than MAX_DEBOUNCE in total, so a driver that keeps firing property-change
                    // notifications cannot starve the refresh.
                    let deadline = Instant::now() + MAX_DEBOUNCE;
                    let mut events = 1u32;
                    let mut pending_refresh = Vec::new();
                    loop {
                        let left = deadline.saturating_duration_since(Instant::now());
                        if left.is_zero() {
                            break;
                        }
                        match rx.recv_timeout(DEBOUNCE.min(left)) {
                            Ok(Msg::Changed) => events += 1,
                            Ok(Msg::Refresh(done)) => pending_refresh.push(done),
                            Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                                return Ok((client, registered.is_ok(), api));
                            }
                            Err(RecvTimeoutError::Timeout) => break,
                        }
                    }
                    eprintln!("shuusshy: {events} device notification(s); re-enumerating");
                    publish(api.snapshot());
                    for done in pending_refresh {
                        let _ = done.send(());
                    }
                }
                Ok(Msg::Refresh(done)) => {
                    publish(api.snapshot());
                    let _ = done.send(());
                }
                Ok(Msg::Shutdown) | Err(_) => return Ok((client, registered.is_ok(), api)),
            }
        }
    })();

    match result {
        Ok((client, registered, api)) => {
            if registered {
                let _ = unsafe {
                    api.enumerator()
                        .UnregisterEndpointNotificationCallback(&client)
                };
            }
            drop(client);
            drop(api);
        }
        Err(msg) => {
            let _ = ready.send(Err(msg));
        }
    }
    unsafe { CoUninitialize() };
}

/// COM callback object. Windows calls these on its own threads; per the API contract they must
/// return quickly and must not call back into the enumerator, so they only post a message.
#[implement(IMMNotificationClient)]
struct Notifier(Sender<Msg>);

impl IMMNotificationClient_Impl for Notifier_Impl {
    fn OnDeviceStateChanged(
        &self,
        _id: &PCWSTR,
        _state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        let _ = self.0.send(Msg::Changed);
        Ok(())
    }
    fn OnDeviceAdded(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.0.send(Msg::Changed);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.0.send(Msg::Changed);
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        _role: ERole,
        _id: &PCWSTR,
    ) -> windows::core::Result<()> {
        let _ = self.0.send(Msg::Changed);
        Ok(())
    }
    fn OnPropertyValueChanged(
        &self,
        _id: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        let _ = self.0.send(Msg::Changed);
        Ok(())
    }
}

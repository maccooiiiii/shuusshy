//! System tray (spec Part 6.7): open the banner, switch noise cancellation, show/hide the
//! banner, open settings, exit. The icon and tooltip show the real processing state, derived
//! from the audio controller's status, never from the toggle alone.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::audio::controller::{RoutingState, RoutingStatus};
use crate::audio::engines::EngineStatus;
use crate::commands::{self, AppState, Audio, MAIN_WINDOW};

/// Event asking the banner to show its settings panel.
pub const OPEN_SETTINGS: &str = "open-settings";
/// Event carrying `true`/`false` when the banner is shown or hidden/minimised. WebView2 does
/// not report a hidden window to the page (document.hidden stays false), so animations use
/// this to pause; measured: the emblem kept costing ~6 % of a core while in the tray.
pub const BANNER_VISIBLE: &str = "banner-visible";

/// Event carrying `Settings` after they changed outside the banner (from the tray).
pub const SETTINGS_CHANGED: &str = "settings-changed";

/// Tray animation frames rendered in Blender (tools/blender/render_emblem.py): the emblem's
/// voice bar pulsing and a ripple radiating while noise cancellation is on. 2 s loop.
const TRAY_FRAMES: [&[u8]; 12] = [
    include_bytes!("../icons/tray/on-00.png"),
    include_bytes!("../icons/tray/on-01.png"),
    include_bytes!("../icons/tray/on-02.png"),
    include_bytes!("../icons/tray/on-03.png"),
    include_bytes!("../icons/tray/on-04.png"),
    include_bytes!("../icons/tray/on-05.png"),
    include_bytes!("../icons/tray/on-06.png"),
    include_bytes!("../icons/tray/on-07.png"),
    include_bytes!("../icons/tray/on-08.png"),
    include_bytes!("../icons/tray/on-09.png"),
    include_bytes!("../icons/tray/on-10.png"),
    include_bytes!("../icons/tray/on-11.png"),
];
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / 6);

/// Windows truncates tray tooltips at 127 characters.
const TOOLTIP_MAX: usize = 127;

/// What the tray icon shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Audio flows and noise suppression is applied: full icon, green dot.
    Active,
    /// Noise cancellation is off (audio passes through) or the virtual mic is off: dimmed icon.
    Off,
    /// Starting, waiting for a device, or interrupted: amber dot.
    Warning,
    /// Failed (or the engine is overloaded and bypassing): red dot.
    Error,
}

/// Tray level and tooltip from the controller's real status.
pub fn presentation(status: &RoutingStatus) -> (Level, String) {
    let (level, text) = match &status.engine {
        EngineStatus::Processing => (
            Level::Active,
            format!(
                "Noise cancellation on ({})",
                status.engine_name.unwrap_or("engine")
            ),
        ),
        EngineStatus::Bypassed => (
            Level::Off,
            "Noise cancellation off (microphone passes through)".to_string(),
        ),
        EngineStatus::Initializing => (Level::Warning, "Starting…".to_string()),
        EngineStatus::Unavailable { reason } => {
            if matches!(status.state, RoutingState::Off) {
                (Level::Off, "Virtual microphone off".to_string())
            } else {
                (Level::Warning, format!("Not processing: {reason}"))
            }
        }
        EngineStatus::Interrupted { reason } => (Level::Warning, format!("Interrupted: {reason}")),
        EngineStatus::Error { reason } => (Level::Error, format!("Problem: {reason}")),
    };
    (level, truncate(&format!("Shuusshy — {text}"), TOOLTIP_MAX))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('…');
    t
}

pub struct Tray {
    icon: TrayIcon<Wry>,
    toggle: CheckMenuItem<Wry>,
    visibility: MenuItem<Wry>,
    /// The Blender frames as rendered (cyan), recoloured into `art` for the accent.
    base_frames: Vec<Image<'static>>,
    art: Mutex<Art>,
    /// True while the icon should animate (processing).
    animating: Arc<AtomicBool>,
    last: Mutex<Option<(Level, String)>>,
}

/// Tray pictures in the current accent colour.
struct Art {
    /// Icons for Active, Off, Warning, Error.
    images: [Image<'static>; 4],
    /// The animation loop shown while processing.
    frames: Vec<Image<'static>>,
}

impl Art {
    fn new(base_frames: &[Image<'static>], accent: [u8; 3]) -> Self {
        let frames: Vec<Image<'static>> = base_frames
            .iter()
            .map(|f| {
                Image::new_owned(
                    crate::appearance::retint(f.rgba(), accent),
                    f.width(),
                    f.height(),
                )
            })
            .collect();
        let base = &frames[0];
        let dimmed = badge(base, None, true);
        let images = [
            base.clone(),
            dimmed.clone(),
            badge(&dimmed, Some([0xff, 0xb8, 0x4d]), false),
            badge(&dimmed, Some([0xff, 0x5d, 0x6c]), false),
        ];
        Self { images, frames }
    }
}

impl Tray {
    fn image(&self, level: Level) -> Image<'static> {
        let art = self.art.lock().unwrap_or_else(|p| p.into_inner());
        art.images[match level {
            Level::Active => 0,
            Level::Off => 1,
            Level::Warning => 2,
            Level::Error => 3,
        }]
        .clone()
    }
}

/// Recolours the tray icon and its animation for a new accent colour.
pub fn set_accent(app: &AppHandle, accent: [u8; 3]) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.try_state::<Tray>() else {
            return;
        };
        *tray.art.lock().unwrap_or_else(|p| p.into_inner()) = Art::new(&tray.base_frames, accent);
        // Forget the last state so the next refresh sets the recoloured icon.
        *tray.last.lock().unwrap_or_else(|p| p.into_inner()) = None;
        refresh_now(&handle);
    });
}

/// Creates the tray icon and its menu.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.audio.suppression_enabled)
        .unwrap_or(false);

    let open = MenuItem::with_id(app, "open", "Open Shuusshy", true, None::<&str>)?;
    let toggle = CheckMenuItem::with_id(
        app,
        "toggle",
        "Noise cancellation",
        true,
        enabled,
        None::<&str>,
    )?;
    let visibility = MenuItem::with_id(app, "visibility", "Hide banner", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit Shuusshy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &visibility,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &exit,
        ],
    )?;

    let base_frames = TRAY_FRAMES
        .iter()
        .map(|b| Image::from_bytes(b).map(|i| i.to_owned()))
        .collect::<tauri::Result<Vec<_>>>()?;
    let accent = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.app.accent)
        .unwrap_or_default();
    let art = Art::new(&base_frames, crate::appearance::accent_rgb(accent));

    let icon = TrayIconBuilder::with_id("main")
        .icon(art.images[2].clone())
        .tooltip("Shuusshy — Starting…")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_banner(app),
            "toggle" => toggle_noise_cancellation(app),
            "visibility" => {
                if banner_visible(app) {
                    hide_banner(app);
                } else {
                    show_banner(app);
                }
            }
            "settings" => {
                show_banner(app);
                let _ = app.emit(OPEN_SETTINGS, ());
            }
            "exit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if banner_visible(app) {
                    hide_banner(app);
                } else {
                    show_banner(app);
                }
            }
        })
        .build(app)?;

    let animating = Arc::new(AtomicBool::new(false));
    spawn_animator(app.clone(), animating.clone(), base_frames.len());
    app.manage(Tray {
        icon,
        toggle,
        visibility,
        base_frames,
        art: Mutex::new(art),
        animating,
        last: Mutex::new(None),
    });
    refresh(app);
    Ok(())
}

/// Steps the tray animation about 6 times a second while processing. Each frame is applied on
/// the main thread without waiting. Holds the first frame when Windows animations are off.
fn spawn_animator(app: AppHandle, animating: Arc<AtomicBool>, n: usize) {
    let _ = std::thread::Builder::new()
        .name("shuusshy-tray-anim".into())
        .spawn(move || {
            let mut frame = 0usize;
            loop {
                std::thread::sleep(FRAME_INTERVAL);
                if !animating.load(Relaxed) || !animations_enabled() {
                    frame = 0;
                    continue;
                }
                frame = (frame + 1) % n;
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Some(tray) = handle.try_state::<Tray>()
                        && tray.animating.load(Relaxed)
                    {
                        let img = tray.art.lock().map(|a| a.frames[frame].clone()).ok();
                        let _ = tray.icon.set_icon(img);
                    }
                });
            }
        });
}

/// macOS: System Settings > Accessibility > Display > "Reduce motion". Called every frame, so
/// the setting is read at most every 5 seconds.
#[cfg(target_os = "macos")]
fn animations_enabled() -> bool {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, on)) = *cache
        && at.elapsed() < Duration::from_secs(5)
    {
        return on;
    }
    let on = std::process::Command::new("defaults")
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() != "1")
        .unwrap_or(true);
    *cache = Some((Instant::now(), on));
    on
}

/// Windows' "Show animations in Windows" setting (Settings > Accessibility > Visual effects).
#[cfg(windows)]
fn animations_enabled() -> bool {
    use windows::Win32::Foundation::TRUE;
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut on = TRUE;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer we pass.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut on as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_err() || on.as_bool()
}

/// Schedules a tray update (icon, tooltip, check mark, show/hide label) from the current state.
///
/// Never blocks the caller: the update runs later on the main thread, which owns the tray.
/// Tray and window calls made from another thread wait for the main thread. The audio
/// controller calls this from its worker thread, so a synchronous update could deadlock
/// against the main thread waiting for a lock the worker holds. (It did: the settings lock
/// was held across `set_checked` while the main thread handled "close to tray".)
pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || refresh_now(&handle));
}

/// Applies the tray update. Main thread only. Unchanged state touches nothing.
fn refresh_now(app: &AppHandle) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.audio.suppression_enabled)
        .ok();
    if let Some(on) = enabled {
        let _ = tray.toggle.set_checked(on);
    }
    let _ = tray.visibility.set_text(if banner_visible(app) {
        "Hide banner"
    } else {
        "Show banner"
    });
    let Some(audio) = app.try_state::<Audio>() else {
        return;
    };
    let (level, tooltip) = presentation(&audio.0.status());
    let mut last = tray.last.lock().unwrap_or_else(|p| p.into_inner());
    if last.as_ref() == Some(&(level, tooltip.clone())) {
        return;
    }
    if last.as_ref().map(|l| l.0) != Some(level) {
        tray.animating.store(level == Level::Active, Relaxed);
        let _ = tray.icon.set_icon(Some(tray.image(level)));
    }
    let _ = tray.icon.set_tooltip(Some(&tooltip));
    *last = Some((level, tooltip));
}

fn toggle_noise_cancellation(app: &AppHandle) {
    match commands::set_suppression_enabled(app, None) {
        Ok(settings) => {
            let _ = app.emit(SETTINGS_CHANGED, settings);
        }
        Err(e) => eprintln!("shuusshy: tray toggle failed: {e}"),
    }
    refresh(app);
}

pub fn banner_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN_WINDOW)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

pub fn show_banner(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    let _ = app.emit(BANNER_VISIBLE, true);
    refresh(app);
}

/// Hides the banner to the tray. Live-test monitoring stops (nobody can see its Stop button);
/// the virtual microphone keeps running.
pub fn hide_banner(app: &AppHandle) {
    if let Some(a) = app.try_state::<Audio>() {
        a.0.monitor_stop();
    }
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.hide();
    }
    let _ = app.emit(BANNER_VISIBLE, false);
    refresh(app);
}

/// The app icon with a status dot in the lower right corner, or dimmed and greyed for "off".
fn badge(base: &Image<'_>, dot: Option<[u8; 3]>, dim: bool) -> Image<'static> {
    let (w, h) = (base.width() as usize, base.height() as usize);
    let mut px = base.rgba().to_vec();
    if dim {
        for p in px.as_chunks_mut::<4>().0 {
            let grey = ((p[0] as u32 * 30 + p[1] as u32 * 59 + p[2] as u32 * 11) / 100) as u8;
            p[0] = grey;
            p[1] = grey;
            p[2] = grey;
            p[3] = (p[3] as u32 * 55 / 100) as u8;
        }
    }
    if let Some(rgb) = dot {
        let r = w.min(h) as f32 * 0.22;
        let ring = r + w.min(h) as f32 * 0.07;
        let (cx, cy) = (w as f32 - ring, h as f32 - ring);
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let i = (y * w + x) * 4;
                if d <= r {
                    px[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                } else if d <= ring {
                    // Dark ring so the dot stays visible on light and dark taskbars.
                    px[i..i + 4].copy_from_slice(&[0x1b, 0x1c, 0x1e, 255]);
                }
            }
        }
    }
    Image::new_owned(px, w as u32, h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(state: RoutingState, engine: EngineStatus) -> RoutingStatus {
        RoutingStatus {
            state,
            input: None,
            output: None,
            info: None,
            stats: None,
            uptime_s: None,
            engine,
            engine_name: Some("DeepFilterNet"),
            engine_telemetry: None,
            chain_telemetry: None,
            monitor: None,
            monitor_error: None,
        }
    }

    #[test]
    fn tray_reflects_real_engine_state() {
        let (l, t) = presentation(&status(RoutingState::Running, EngineStatus::Processing));
        assert_eq!(l, Level::Active);
        assert_eq!(t, "Shuusshy — Noise cancellation on (DeepFilterNet)");

        let (l, _) = presentation(&status(RoutingState::Running, EngineStatus::Bypassed));
        assert_eq!(l, Level::Off);

        let (l, t) = presentation(&status(
            RoutingState::Off,
            EngineStatus::Unavailable { reason: "x".into() },
        ));
        assert_eq!(
            (l, t.as_str()),
            (Level::Off, "Shuusshy — Virtual microphone off")
        );

        // A missing device is a problem to fix, not "off".
        let (l, t) = presentation(&status(
            RoutingState::Waiting {
                reason: "VB-CABLE is not installed".into(),
            },
            EngineStatus::Unavailable {
                reason: "VB-CABLE is not installed".into(),
            },
        ));
        assert_eq!(l, Level::Warning);
        assert!(t.contains("VB-CABLE is not installed"));

        // An overloaded engine is an error even though the toggle is on.
        let (l, _) = presentation(&status(
            RoutingState::Running,
            EngineStatus::Error {
                reason: "too slow".into(),
            },
        ));
        assert_eq!(l, Level::Error);
    }

    #[test]
    fn tooltip_fits_windows_limit() {
        let long = "a".repeat(300);
        let (_, t) = presentation(&status(
            RoutingState::Error {
                message: long.clone(),
            },
            EngineStatus::Error { reason: long },
        ));
        assert_eq!(t.chars().count(), TOOLTIP_MAX);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn badge_draws_dot_and_dims() {
        let base = Image::new_owned(vec![200; 32 * 32 * 4], 32, 32);
        let active = badge(&base, Some([1, 2, 3]), false);
        // Lower-right dot centre has the dot colour; top-left is untouched.
        let c = ((32 - 10) * 32 + (32 - 10)) * 4;
        assert_eq!(&active.rgba()[c..c + 4], &[1, 2, 3, 255]);
        assert_eq!(&active.rgba()[0..4], &[200, 200, 200, 200]);
        let off = badge(&base, None, true);
        assert_eq!(off.rgba()[3], 110);
    }

    #[test]
    fn tray_frames_are_32px_and_visible() {
        for (i, bytes) in TRAY_FRAMES.iter().enumerate() {
            let img = Image::from_bytes(bytes).unwrap();
            assert_eq!((img.width(), img.height()), (32, 32), "frame {i}");
            let opaque = img.rgba().chunks(4).filter(|p| p[3] > 128).count();
            assert!(
                opaque > 200,
                "frame {i} is mostly transparent ({opaque} px)"
            );
        }
    }
}

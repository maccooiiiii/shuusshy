# Shuusshy Architecture

_Last updated: Milestone 3._

## Overview

```
┌────────────── WebView (React/TS) ──────────────┐
│  Floating banner UI (src/App.tsx)               │
│  - derives every indicator from backend state   │
└──────────────┬──────────────────────────────────┘
               │ Tauri IPC (validated commands)
┌──────────────▼──────────────── Rust backend ────┐
│ commands.rs   IPC surface, argument validation   │
│ settings/     typed model + SQLite store         │
│ audio/        (Milestone 2+) devices, capture,   │
│               pipeline, engines, routing, meters │
└──────────────────────────────────────────────────┘
```

The audio engine will live entirely in Rust, on its own threads. The UI only sends commands and
receives state and meter snapshots. It never touches audio buffers.

## Window

- One frameless, non-resizable, always-on-top (configurable) window labelled `main`, 360 px wide.
- The frontend measures its content with a `ResizeObserver`. It calls `resize_banner(height)`,
  and the backend clamps the height to 96–640 px. Width is fixed in Rust, so a compromised page
  cannot resize the window arbitrarily.
- Window dragging uses `data-tauri-drag-region` on the header.
- Compact/expanded mode is a setting (`app.compact`); the window height follows automatically.
- Closing: with `app.keepRunningInTray` (default on) the backend intercepts `CloseRequested`,
  hides the window and stops live-test monitoring; the audio keeps running. Without a working
  tray icon the window is never hidden (it could not be brought back).

## Installation (`install.rs`, `installer/`, `scripts/package-portable.ps1`)

- **Portable mode:** `shuusshy.portable` next to the exe → settings in `<exe>\data`, and
  `WEBVIEW2_USER_DATA_FOLDER` set to `<exe>\data\webview` first thing in `run()`.
- **Start with Windows:** the HKCU `…\CurrentVersion\Run` value "Shuusshy" =
  `"<exe>" --autostart`. Its state is read from Windows, not stored in settings. With
  `--autostart` the window (created hidden) stays in the tray.
- **Single instance:** `tauri-plugin-single-instance`; a second launch shows the existing
  banner. Helper modes (`--set-mic-label`, `--restore-mic-name`) are handled before the plugin
  runs.
- **NSIS hooks** (`installer/hooks.nsh`): on a real uninstall (not `/UPDATE`), run
  `--restore-mic-name` (interactive only) and delete the Run value.

## Appearance (`appearance.rs`, `src/lib/appearance.ts`, `index.css`)

- The window is created transparent (`tauri.conf.json`). `appearance::apply_window` sets the
  OS effect (Blur < 22000 ≤ Mica < 22621 ≤ Acrylic) or none. The page paints its own tint
  (`--tint`) so the transparency slider behaves the same with every effect.
- The accent is a CSS variable (`--color-accent`) set at runtime from `get_accent_color`.
  "System" reads `DwmGetColorizationColor`, lifted to lightness ≥ 0.62. The emblem uses a CSS
  hue-rotate/saturate from the render cyan, and `tray::set_accent` recolours the Blender
  frames in HSL.
- Width is user-resizable (340–560 px). `resize_banner` keeps the current width, lifts the
  old size limits, resizes, then pins the height to the content again.

## Licensing (`license.rs`, `license_format.rs`)

Offline Ed25519 keys; see [LICENSING.md](LICENSING.md). `processing_from` runs RNNoise and
keeps isolation off unless the stored key verifies; `apply_preset` refuses Pro presets.
`activate_license` / `remove_license` are the only ways to set the key (`update_settings`
preserves it).

## Virtual mic name (`audio/devices/rename.rs`)

Renames VB-CABLE's recording endpoint description (PKEY_Device_DeviceDesc) between
"CABLE Output" and "Shuusshy Microphone". When the direct write is denied, the app re-runs
itself elevated (`--set-mic-label <id> <label>`). That copy handles the flag first thing in
`run()`, re-validates the ID (well-formed, VB-CABLE's primary capture endpoint) and the label
(one of the two), writes, and exits.

## System tray (`src-tauri/src/tray.rs`)

- Menu: Open Shuusshy, Noise cancellation (check item), Show/Hide banner, Settings…, Exit.
  The tray toggle persists the setting, applies it to the engine and emits `settings-changed`
  so the banner updates; "Settings…" shows the banner and emits `open-settings`.
- Icon and tooltip come from the audio controller's real status (`tray::presentation`), never
  from the toggle alone: green dot = processing, dimmed = off, amber = starting/waiting/
  interrupted, red = error. Badges are drawn at startup from the 32 px app icon.
- **Threading rule:** tray and window calls from other threads block until the main thread
  runs them. `tray::refresh` therefore only schedules the update with `run_on_main_thread` and
  never waits. The audio worker calls it on every status change. A synchronous version
  deadlocked while closing to the tray: the worker held the settings lock while waiting for the
  main thread, and the main thread waited for that lock.

## Security model

- **Capabilities** (`src-tauri/capabilities/default.json`): only `core:event:default` and the
  window `start-dragging`, `minimize` and `close` permissions. No filesystem, shell, HTTP or
  opener plugins. Always-on-top and resizing happen through our own validated commands, not
  through broad window permissions.
- **CSP**: `default-src 'self'`; no remote scripts, no inline scripts; IPC only.
- **Command validation**: `Settings::validate()` checks numeric ranges (finite, bounded) and
  device IDs (non-empty, ≤512 chars, no control chars) before anything is persisted.

## Settings storage

- SQLite file `shuusshy.db` in the per-user app data dir
  (`%APPDATA%\com.shuusshy.app\`). Portable mode (Milestone 9) will redirect this.
- Schema is versioned with `PRAGMA user_version` through an ordered migration list. A database
  from a newer build is refused rather than silently downgraded.
- Each section (`app`, `audio`) is stored as one JSON row. All fields have serde defaults, so
  older or partial data still loads. Unreadable JSON falls back to defaults for that section only.
- If the database cannot be opened, the app runs with an in-memory store and the UI shows a
  "settings won't be saved" warning.
- `audio` settings are persisted now so the schema is stable. Only the microphone selection
  (`inputDeviceId`, `inputDeviceName`) is used so far (Milestone 2).
- `update_settings` never changes the device selection. That goes only through
  `select_input_device`, which verifies the device is an active **physical** microphone.

## Audio Device Manager (Milestone 2)

`src-tauri/src/audio/devices/`

| File | Role |
|---|---|
| `model.rs` | Pure, unit-tested logic: classification, VB-CABLE identification, microphone status, auto-selection |
| `wasapi.rs` | MMDevice API: enumerate active endpoints, read properties, open each endpoint's `IAudioClient` to read its mix format (proves the device can actually be opened), endpoint states |
| `monitor.rs` | Dedicated `shuusshy-devices` thread (COM MTA). Registers an `IMMNotificationClient` and owns the latest snapshot |
| `mod.rs` | `DeviceState` sent to the UI |

**Classification** uses the endpoint's PnP enumerator (`PKEY_Device_EnumeratorName`) and adapter
name (`PKEY_DeviceInterface_FriendlyName`), not just the display name:

- adapter contains `VB-Audio` → `vbCable`
- enumerator `ROOT` / `SW` / `SWD`, or a known virtual adapter (Krisp, Voicemeeter, NVIDIA
  Broadcast, Steam Streaming) → `otherVirtual`
- everything else (`HDAUDIO`, `USB`, `BTHENUM`, `BTHHFENUM`, …) → `physical`

Only physical microphones can be selected. Selecting `CABLE Output` would feed Shuusshy's own
output back into it, and other virtual mics would double-process the audio.

**VB-CABLE**: the virtual mic is ready when both `CABLE Input` (render, where Shuusshy will write)
and `CABLE Output` (capture, what apps record from) are active. Matching uses the endpoint
description, so `CABLE In 16ch` and other VB-Audio products are ignored.

**Hot-plug flow**:
1. Windows calls the notification client on its own thread. The callback only posts a message,
   because the API forbids calling back into the enumerator from inside it.
2. The monitor thread waits for a 250 ms pause in notifications, capped at 1 s total so a
   chatty driver can't delay the refresh forever.
3. It re-enumerates, and only if the snapshot actually changed does it publish and emit
   `devices-changed` with a fresh `DeviceState`.

**Microphone status**: `notSelected`, `available`, `error` (listed but its audio client could not
be opened, with a readable reason such as exclusive use or privacy settings), and `disconnected`
(selected but not active, with the Windows state: unplugged / disabled / not present). The
selection is kept while disconnected, and it becomes `available` again automatically when the
device returns.

**First run**: the Windows default capture device is auto-selected if it is physical; otherwise
the first physical microphone. A virtual device is never auto-selected.

## Audio pipeline and controller (Milestone 3)

See [AUDIO_ENGINE.md](AUDIO_ENGINE.md) for the signal path, drift compensation, recovery rules
and the hardware verification tools. In short:

- `audio/pipeline/` handles capture, the DSP thread, output, drift control and counters.
- `audio/controller.rs` owns the pipeline on its own thread and reconciles it with settings and
  device state.
- The UI receives a `routing-changed` event on state changes and polls `get_routing_status` once
  a second for the live counters (buffer, drift, dropouts).
- `routingEnabled` (default on) lets the user stop sending audio to the virtual microphone.

## Planned audio design (from Milestone 0 assessment)

| Concern | Plan |
|---|---|
| Device I/O | `cpal` 0.18 (WASAPI) for streams, opened by the same endpoint ID (`DeviceId` = `wasapi:<endpoint id>`). Enumeration and hot-plug stay in `audio/devices` |
| Frame format | 48 kHz mono, 480-sample (10 ms) frames — native to both RNNoise and DeepFilterNet |
| Resampling | `rubato` at capture and output boundaries |
| Clock drift | Adaptive ring buffer between mic and VB-CABLE clocks (Milestone 3 exit criterion) |
| Real-time safety | Lock-free SPSC buffers, pre-allocated frames, atomics for parameters |
| RNNoise | `nnnoiseless` (pure Rust). No native strength control, so strength is a documented wet/dry mix |
| DeepFilterNet | `deep_filter` (tract). `atten_lim_db` maps to strength; feasibility spike first |

## Open decisions

1. **License.** MIT or Apache-2.0 for source is recommended. The two-year expiry (spec Part 10)
   can only bind official binaries, as a transparent build feature. Legal review is needed if
   binary terms are wanted.
2. **GitHub repository URL.** Needed before configuring `origin`.

# Changelog

All notable changes are documented here. Format: [Keep a Changelog](https://keepachangelog.com/),
versions: [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0] - 2026-11-16

First stable release. Valid until 2028-11-16. Release notes: docs/RELEASE_NOTES_1.0.0.md;
known limitations: docs/KNOWN_LIMITATIONS.md.

### Highlights
- Local, offline noise cancellation for any microphone, through a virtual microphone (VB-CABLE).
- RNNoise (free) and DeepFilterNet (Pro) with adjustable strength and a genuine bypass; voice
  isolation (Pro); presets.
- Live test with real meters and headphone monitoring (nothing recorded).
- Liquid-glass banner, accent colours, animated tray icon, start with Windows.
- Per-user installer, portable ZIP, signed two-year validity metadata, settings export/import.
- Update checks against the official GitHub repository (optional, signed updates).

### Development history (milestones 0–11)

### Added
- QA tools: `tools/qa-record` (records mic and virtual mic together, with a channel phase
  check) and `examples/voice_variants` (replays a recording through the chain in several
  configurations).
- Milestone 10: **releases, updates and validity period**.
  - Signed release metadata (Ed25519, verified offline). Each official release is valid for
    two years: a reminder 30 days before, a warning 7 days before, and on expiry processing
    and the virtual microphone stop (settings kept, download page and settings export
    available). Builds without metadata show as "Unofficial build".
  - Update system: check GitHub Releases (on start-up at most daily, or manually), release
    notes, download with progress, updater-signature verification, install on click; optional
    automatic checking and downloading; the portable edition links to the Releases page.
  - Settings export and import (JSON, without the Pro key or device IDs).
  - GitHub Actions: CI with build validation; tag-triggered release workflow producing a
    draft release with the installer, updater signature, `latest.json`, the portable ZIP and
    SHA256SUMS.
  - LICENSE (source-available), PRIVACY, SECURITY, CONTRIBUTING, INSTALLATION,
    docs/RELEASES.md; an installer terms page discloses the validity period.
- Milestone 9: **installer and portable edition**.
  - NSIS installer: per-user, no admin rights; Start menu shortcut, optional desktop shortcut,
    upgrades over a running copy (settings, Pro key and Start with Windows kept), uninstaller
    that removes the Run entry and offers to rename the virtual mic back. Branded artwork.
  - Portable ZIP (`npm run package:portable`): the `shuusshy.portable` marker keeps settings
    and the WebView2 cache in `data\` beside the exe; a Portable badge in Settings.
  - **Start with Windows** (starts hidden in the tray), single-instance (a second launch shows
    the banner), "Open in browser" for the VB-CABLE download page.
  - New app icon rendered from the Blender emblem; SHA-256 files for both artifacts;
    `docs/INSTALL.md`.
- **Credits**: Settings → About ends with a small "Created by Ian Musa · © 2026" line, with app info and the
  projects Shuusshy builds on. Ian Musa is also set as author (Cargo), publisher and
  copyright holder (Windows Apps list, installer), on the installer's side panel and in the
  README.
- Milestone 1: Tauri 2 + React + TypeScript + Vite + Tailwind desktop shell.
- Frameless 360 px floating banner with drag, minimize, close and content-fitted height.
- Settings panel with a working "Keep banner on top" option, version and data folder.
- SQLite settings store with schema migrations, validation and reset.
- Restrictive Tauri capabilities and Content Security Policy.
- CI workflow: typecheck, Vitest, frontend build, rustfmt, clippy, cargo test (windows-latest).
- Milestone 2: audio device manager (WASAPI/MMDevice via the `windows` crate).
  - Microphone list with real per-device format, Windows-default marker and open errors.
  - Physical/virtual classification; only physical microphones are selectable.
  - Persisted selection with first-run auto-selection of the default physical microphone.
  - Disconnected / error / available microphone status that recovers automatically.
  - VB-CABLE detection (ready / incomplete / not installed) with setup guidance.
  - Hot-plug notifications (`IMMNotificationClient`) with bounded debounce.
  - docs/TROUBLESHOOTING.md.
- Milestone 3: real-time pass-through from the selected microphone to VB-CABLE ("CABLE Input").
  - cpal/WASAPI streams, lock-free `rtrb` rings, MMCSS DSP thread, rubato resampling,
    48 kHz / 480-sample frames ready for noise-suppression engines.
  - Margin-based clock-drift compensation (PI controller), priming, underrun recovery and
    latency-capping resync.
  - Audio controller thread: reconciles routing with settings and devices; recovers from stream
    errors and stalls with backoff; counts WASAPI glitch notices separately.
  - "Send microphone to virtual mic" setting (default on); live routing status and counters.
  - Hardware verification tools: `examples/route_check.rs` (cross-correlation routing proof,
    latency, soak, CPU-load test), `examples/cable_latency.rs`, and `examples/mic_level.rs`
    (read-only mute/volume/signal-level check per input).
  - docs/AUDIO_ENGINE.md.

- Virtual-cable conflict detection: every 2 s Shuusshy lists apps with active audio sessions on
  "CABLE Input" / "CABLE In 16ch" and warns by name (e.g. an app whose speaker is set to the
  cable), in the banner and Settings. The "cable busy" error (AUDCLNT_E_DEVICE_IN_USE) is now
  explained in plain words.
- Diagnostics: `examples/who_uses.rs` (audio sessions per endpoint) and
  `examples/cable_occupy.rs` (plays silence into the cable to reproduce a conflict).

- Milestone 4: **RNNoise noise suppression** (nnnoiseless, BSD-3-Clause).
  - Working "Noise cancellation" switch (genuine bit-exact bypass) and strength slider
    (documented wet/dry mix), both live without restarting audio, click-free one-frame ramps.
  - Real engine status in the banner: Active / Bypassed / Starting / Unavailable (with reason) /
    Interrupted / Error. Engine CPU time and voice-activity shown in Settings.
  - "Noise Suppression" settings section with engine choice (DeepFilterNet shown as coming in
    Milestone 5).
  - `examples/denoise_check.rs`: measures noise removed and speech kept on real microphone audio.
  - THIRD_PARTY_LICENSES.md.

- Milestone 5: **DeepFilterNet** engine (dfn3_rs, DeepFilterNet3, MIT OR Apache-2.0).
  - Chosen over the tract-based port after a spike: 4x faster, no allocations, no 200 ms spikes.
  - Strength = DeepFilterNet's own attenuation limit; exact bypass; 30 ms delay.
  - Overload watchdog: falls back to untouched audio and reports "too slow" instead of glitching.
  - Engine selection in Settings (restarts audio with the chosen engine); engine-specific
    strength explanation and figures (voice activity / speech-to-noise).
  - `examples/engine_compare.rs` (both engines vs clean speech) and both engines in
    `denoise_check`.
- Audio libraries are compiled optimized even in development builds.
- Milestone 6: **audio controls and presets**.
  - Input and output gain (±24 dB), ramped click-free; applied live.
  - Optional noise gate with threshold, 4 dB hysteresis and 200 ms hold; live open/closed state.
  - Safety limiter at −0.5 dBFS with a warning when it is working.
  - Presets: Balanced, Strong Noise Reduction, Speech Priority, Low CPU Usage, plus automatic
    Custom; the active preset is named in the banner.
  - `examples/levels_check.rs` verifies gains and gate on hardware.
- **Voice isolation**: mutes background speech quieter than the user's own voice while they
  pause (adaptive gate keyed to a tracked voice level, with zero-latency speech-aware
  look-ahead). Benchmark: background talker −22 dB in pauses, residual noise −9 dB, voice level
  unchanged, ~1 % of voice blocks still attenuated. Adjustable margin (3–15 dB, default 6).
  The "Strong Noise Reduction" preset now uses voice isolation instead of a fixed gate.
- **Liquid-glass redesign**:
  - glassmorphism surfaces with specular rims and a pointer-following highlight;
  - liquid switch and sliding lens;
  - optional see-through glass window with a transparency slider (Blur/Mica/Acrylic by Windows
    version);
  - seven accent colours plus the Windows accent, applied to the UI, emblem and tray icon;
  - a resizable width (340–560 px) with a two-column settings layout.
- **Design refresh** (superseded by the glass redesign): futuristic HUD styling, motion system (reduced-motion aware), Blender-rendered
  animated emblem in the banner and an animated tray icon while noise cancellation is
  processing (`tools/blender/render_emblem.py`).
- **"Shuusshy Microphone"**: optional rename of VB-CABLE's recording endpoint so Teams,
  Discord and Zoom list it under Shuusshy's name (Windows admin prompt; reversible).
- **Shuusshy Pro**: offline Ed25519 license keys (`tools/license-keygen`). RNNoise stays free;
  DeepFilterNet, voice isolation and their presets need a key. First-run activation card.
- Milestone 8: **final banner and system tray**.
  - Compact / expanded banner (remembered), engine selector and activity bar in the expanded
    banner, entrance transitions (respecting reduced motion), dismissible errors.
  - Settings reorganised into the spec's seven sections; the noise gate and connection details
    are folded away as advanced.
  - System tray with Open, Noise cancellation, Show/Hide banner, Settings and Exit. The icon
    and tooltip show the real processing state.
  - "Keep running in the tray when closed" (default on).
- Milestone 7: **live TEST panel**.
  - Real before/after meters (RMS + peak hold) measured on the audio thread; live activity bar
    in the banner.
  - Headphone monitoring of the original or processed signal (never both), time-aligned, with
    instant Stop (button, Esc, closing the panel). VB-CABLE is excluded; feedback warning for
    loudspeaker-like devices. Nothing is recorded.
  - Monitoring runs as its own playback pipeline and never interrupts the virtual microphone.
  - `examples/monitor_check.rs` (loopback verification tool).
- `examples/isolation_bench.rs` (noise + background-voice benchmark). Candidates rejected on
  measurement: DeepFilterNet→RNNoise cascade (more distortion) and the hush-vani model
  (attenuates all speech by ~14 dB, 16 kHz only; dev-dependency for the benchmark, not shipped).

### Fixed
- The "safety limiter is reducing peaks" warning now clears after a gain change (it used to
  count since start-up).
- Safety limiter ceiling lowered to −1.0 dBFS: the clock-drift resampler after it could push
  peaks to −0.1 dBFS (Milestone 11 QA).
- Interface CPU use cut from ~38 % to ~5–7 % of a core while live (frame-efficient emblem,
  static glow and background light), and to ~0.7 % while hidden in the tray (Milestone 11 QA).
- Closing the banner to the tray could freeze the app (tray update deadlock found during
  Milestone 8 testing, fixed before release).
- Settings panel content no longer gets cut off when taller than the banner's maximum height.

### Security
- `update_settings` can no longer change the selected device; selection goes only through
  `select_input_device`, which rejects virtual and inactive devices.

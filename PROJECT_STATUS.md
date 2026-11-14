# Shuusshy — Project Status

_Last updated: 2026-11-13_

| Milestone | Status |
|---|---|
| 0 — Environment inspection | ✅ Complete ([report](docs/MILESTONE_0_ENVIRONMENT.md)) |
| 1 — Desktop foundation | ✅ Complete |
| 2 — Device management | ✅ Implemented — **hot-plug not yet verified on hardware** |
| 3 — Audio pass-through + drift handling | ✅ Complete (verified in Discord by the user) |
| 4 — RNNoise | ✅ Complete |
| 5 — DeepFilterNet | ✅ Complete |
| 6 — Audio controls & presets | ✅ Complete (+ voice isolation) |
| 7 — Live test & meters | ✅ Complete |
| 8 — UI/UX polish, tray | ✅ Complete (tray confirmed by the user) |
| 8b — Design refresh, Pro licensing, mic name, glass redesign, credits | ✅ Complete (mic rename used by the user) |
| 9 — Installer & portable | ✅ Implemented; install, upgrade, uninstall and portable verified on the dev PC; awaiting review |
| 10 — GitHub, updates, expiration | Not started |
| 11 — QA | Not started |
| 12 — 1.0.0 release | Not started |

## What works today

- **Banner (Milestone 8):**
  - Compact and expanded modes, switched from the header and remembered. Compact shows
    identity, status, microphone, on/off and TEST. Expanded adds the activity bar, an engine
    selector and the strength slider.
  - Drag, minimize, close; height follows content; settings scroll above 640 px.
  - Subtle entrance transitions, switched off when Windows asks for reduced motion.
  - Dismissible error messages and an announced status pill.
- **Settings in the spec's 7 sections:** Microphone (+ input gain), Noise Suppression
  (advanced noise gate folded away), Audio Output (virtual mic + output gain), Testing and
  Monitoring, Startup and Behavior, Application and Updates, About.
- **System tray (Milestone 8):**
  - Menu: Open Shuusshy, Noise cancellation (check mark), Show/Hide banner, Settings…, Exit
    Shuusshy. Left click shows or hides the banner.
  - The icon shows the real engine state: green dot = processing, dimmed = off, amber =
    starting/waiting/interrupted, red = error. The tooltip says what is happening.
  - "Keep running in the tray when closed" (default on): closing the banner hides it and the
    virtual microphone keeps working; turned off, closing exits. Hiding stops live-test
    monitoring.
- Microphone list, physical/virtual classification, persisted selection, first-run
  auto-selection, disconnected/error status, VB-CABLE detection (Milestone 2).
- **Virtual microphone routing (Milestone 3):** while Shuusshy runs, the selected microphone is
  sent **unprocessed** to VB-CABLE, so other apps receive it by choosing "CABLE Output".
  - Real-time pipeline: lock-free ring buffers, MMCSS "Pro Audio" DSP thread, 48 kHz / 10 ms
    frames, resampling at both ends.
  - Clock-drift compensation, so latency does not grow and the buffer does not drain over time.
  - Automatic start/stop/restart on device changes, stream errors and stalls, with retry backoff.
  - "Send microphone to virtual mic" switch (default on) in Settings.
  - **Virtual-cable conflict warning**: names any other app playing into the cable (for example
    an app whose speaker is set to "CABLE Input"), and explains the "cable busy" error.
  - Live status in the banner and detailed counters in Settings (buffer, drift, dropouts,
    overflows, resyncs, device glitches).
- "Keep banner on top" setting.

- **RNNoise noise suppression (Milestone 4):** working Noise cancellation switch (genuine
  bypass) and Strength slider (documented wet/dry mix), applied live and click-free; real engine
  status in the banner (Active / Bypassed / Starting / Unavailable / Interrupted / Error); engine
  CPU time and voice activity in Settings.

- **Live TEST (Milestone 7):**
  - real before/after meters;
  - headphone monitoring of the original or processed signal, with instant Stop/Esc;
  - feedback warning; banner activity bar.
- **Voice isolation:** mutes background speech that is quieter than your own voice while you
  pause; adjustable margin; live readout of your tracked voice level and the gate.
- **Audio controls (Milestone 6):** input/output gain, optional noise gate with live state,
  safety limiter, and presets (Balanced, Strong Noise Reduction, Speech Priority, Low CPU Usage,
  Custom), all applied live.

## What does NOT work yet (shown as disabled in the UI)

- **DeepFilterNet (Milestone 5):** selectable engine with real strength control (attenuation
  limit), exact bypass, and an overload watchdog. Switching engines restarts audio briefly.

## Milestone 9 verification (2026-11-14)

Artifacts (`src-tauri/target/release/bundle/`), each with a `.sha256` file:
- `nsis/Shuusshy_0.1.0_x64-setup.exe` (7.9 MiB)
- `portable/Shuusshy_0.1.0_x64_portable.zip`

| Check | Result |
|---|---|
| Install (silent, per-user) | Exit 0, **no admin prompt**. Installs to `%LOCALAPPDATA%\Shuusshy`. Start menu shortcut, desktop shortcut (silent installs add it; interactive installs make it a finish-page option), Apps-list entry "Shuusshy 0.1.0, publisher Ian Musa" |
| Installed app | Ran with all existing settings and the Pro key; a second launch did not start another copy |
| Start with Windows | The switch wrote `"…\shuusshy.exe" --autostart` to HKCU Run. A launch with `--autostart` stayed hidden in the tray, and its session on CABLE Input was ACTIVE |
| Upgrade 0.1.0 → 0.1.1 (test build) | Silent install over the running app: the old copy was closed, the version became 0.1.1, and the Start with Windows entry, settings and Pro key were all kept (DeepFilterNet + preset active after launch). The first hook draft would have deleted the Run entry on upgrade; fixed by skipping `/UPDATE` runs |
| Uninstall (silent) | App closed; program, Start menu and desktop shortcuts, Run entry and Apps entry removed; settings kept (as designed: interactive uninstall offers "delete application data") |
| Portable ZIP | Ran from an extracted folder. `data\` (settings DB + WebView2 cache) was created next to the exe, with **no writes** to `%APPDATA%` or `%LOCALAPPDATA%`. Settings show the **Portable** badge and the data path. Fresh settings (no Pro key) |
| Not exercised | Interactive installer pages; interactive uninstall with "rename the mic back" (needs the admin prompt); the WebView2 download on a PC without WebView2 |

## Liquid-glass redesign (2026-11-14)

The HUD look is replaced by a clean glassmorphism design inspired by liquid glass:
- **Surfaces.** Translucent panels with blur, a specular rim lit from the top left, and a
  highlight that follows the pointer.
- **Controls.** Lens-like switch knob that squishes when pressed; a glass lens slides between
  the engine choices.
- **Motion.** Springy, with blur-in view transitions and slowly drifting accent light behind
  the panels. All motion stops for reduced motion.

| Feature | How |
|---|---|
| **Glass window** (Settings → Appearance) | The window is transparent. Windows blurs the desktop behind it: Blur on Windows 10, Mica on early Windows 11, Acrylic on 22H2+ (Acrylic drags poorly on Windows 10). A **Transparency** slider (5–65 %) sets the dark tint over it. On by default; turning it off gives an opaque, rounded banner |
| **Accent colours** | Cyan, Violet, Mint, Amber, Rose, Ice, or the **Windows accent** (dark accents lifted for contrast). Recolours highlights, the Blender emblem (hue shift) and the **tray icon frames** (recoloured in Rust) |
| **Responsive** | The banner can be dragged from 340 to 560 px wide; height follows the content. From 500 px, Settings flows into two columns |

| Check | Result |
|---|---|
| Unit tests | Colour maths (HSL round trip, tray recolouring keeps greys/alpha, dark accents lifted), effect per Windows build, emblem hue filter, tint validation; 72 Rust + 24 frontend tests |
| Real app | Glass window on Windows 10 (Blur): desktop visible and blurred behind the panels. Main view and settings in the new style; 520 px width gave two settings columns. Found and fixed: the height lock clamped resizes until the old limits were lifted first |
| Mic rename | Used by the user: the app shows "Shuusshy Microphone" |
| Not checked by me | Accent switching and the transparency slider by hand (the user was using the app); tray recolouring is unit-tested only |

## Design refresh, Pro licensing and mic name (2026-11-14)

Requested mid-Milestone 9.

- **Futuristic look.** Dark HUD style:
  - glass panels with hairline borders and corner ticks, a faint grid background;
  - one electric-cyan accent that lights up only for real activity;
  - Bahnschrift / Cascadia Mono system fonts (no web fonts: the app stays offline);
  - press feedback, spring switch, sliding engine selector, view slide-in, staggered settings.
  All motion respects Windows' reduced-motion setting.
- **Blender effects.** `tools/blender/render_emblem.py` renders a 3D emblem in Blender 5.3
  (Cycles), with bloom added in numpy. While noise cancellation is **really** processing:
  - the voice bar pulses, the noise bars stay "shushed" and a ripple radiates (banner, 36-frame
    loop);
  - the **tray icon animates** with 12 Blender frames at about 6 fps. It holds still if Windows
    animations are off.
  - Turning on adds one light sweep across the card.
- **"Shuusshy Microphone" in Teams/Discord/Zoom.** Settings → Audio Output can rename
  VB-CABLE's recording endpoint between "CABLE Output" and "Shuusshy Microphone":
  - apps then list "Shuusshy Microphone (VB-Audio Virtual Cable)";
  - Windows asks for administrator permission; the elevated helper accepts only those two
    names for VB-CABLE's own endpoint;
  - detection still finds the cable after renaming.
- **Shuusshy Pro (offline keys).** RNNoise is free. DeepFilterNet, voice isolation and their
  presets need a key. See [docs/LICENSING.md](docs/LICENSING.md).

| Check | Result |
|---|---|
| Unit tests | Key format (base32, dates, tolerant parsing); signature: valid, tampered, foreign, expired, wrong edition, garbage; Pro gating; renamed cable still detected; rename accepts only the two names and well-formed endpoint IDs; tray frames 32 px and visible |
| Real app: look | Off and live states captured; the emblem animates only while processing; settings in the new style |
| Real app: licensing | Without a key, RNNoise ran although DeepFilterNet was stored; PRO badges and the first-run card showed. The issued key (serial 1) was typed in and activated; the pipeline restarted with DeepFilterNet |
| Mic rename | **Not exercised**: it needs the user to approve the Windows administrator prompt |
| Tray animation | Frames verified in tests and visually; the live tray icon sits in the overflow area, so it was not watched animating |

## Milestone 8 verification (2026-11-14)

| Check | Result |
|---|---|
| Unit tests | Tray state follows the real engine status (processing, bypassed, virtual mic off, waiting for VB-CABLE, overloaded engine = error); tooltip cut to Windows' 127-character limit; tray badge pixels; old settings files load with the new defaults |
| Real app: banner | Expanded → compact → expanded from the header; only the layout changed (audio settings unchanged). Settings show all 7 sections |
| Real app: close to tray | First attempt **froze the app** (deadlock between the audio worker and the main thread over the tray update). Fixed: tray updates now always run asynchronously on the main thread. Retested: banner hidden, app responsive, tray icon registered |
| Virtual mic while hidden | `mic_level` on CABLE Output while the banner was hidden: peaks −31 dBFS (audio flowing; the silent stretches are voice isolation muting room noise) |
| Tray menu | Not exercised automatically (the icon sits in the hidden-icons overflow); **checked by hand by the user: works** |
| clippy, rustfmt, tests, build | Clean; 59 Rust tests + 1 hardware test; 21 frontend tests; release build OK |

## Milestone 7 verification (2026-11-14)

| Check | Result |
|---|---|
| Unit tests | Meters exact for a known sine (−23.0 dBFS RMS, −20.0 dBFS peak; output follows −10 dB gain); peak falls 15 dB/s; silence → floor; original and processed monitor streams sample-identical through a delaying engine; off writes nothing; replaced monitor rings returned to the controller, not freed on the audio thread |
| Real app: meters | Room noise −54 dB before suppression, "—" after; speech −31 dB before, −41 dB after |
| Real app: monitoring | On the user's Razer headset (8 ch): original → processed → Stop (Esc). User heard both, then silence. Virtual mic stayed live; 0 monitor dropouts shown |
| Automated loopback check | Inconclusive on this device: WASAPI loopback of the Razer virtual surround endpoint returned digital silence while audio was audible |
| clippy, rustfmt, tests, build | Clean; 56 Rust tests + 1 hardware test; 20 frontend tests; build OK |

## Voice isolation verification (2026-11-14)

| Check | Result |
|---|---|
| Benchmark (TTS voice + background talker + fan + clicks, 4 scenarios) | Default 6 dB margin: background speech in pauses −21.8 / −22.7 dB (talker 12 dB quieter), −8.2 dB (9 dB quieter), −2.0 dB (6 dB quieter); voice level unchanged; SI-SDR better than DeepFilterNet alone in all scenarios; ~1 % of voice blocks attenuated >6 dB |
| Rejected on measurement | DeepFilterNet→RNNoise cascade (SI-SDR 9.7 → 7.7 dB); hush-vani (all speech −14 dB, 16 kHz); level-only and speech-only look-ahead variants |
| Unit tests | Voice tracker rise/decay; isolation mutes a 12 dB quieter talker by >60 dB and passes the voice bit-exact; look-ahead opens early only 6 dB above threshold; disabled chain bit-exact with a delaying engine |
| Real app | Strong Noise Reduction preset → isolation on (6 dB); live readout: voice ≈ −25 dBFS, muting below −31 dBFS, gate closed in a pause |
| clippy, rustfmt, tests | Clean; 53 Rust tests + 1 hardware test; 17 frontend tests |

## Milestone 6 verification (2026-11-14)

| Check | Result |
|---|---|
| Unit tests (chain) | Neutral chain bit-exact; gains within 0.05 dB; ramps without steps; gate mutes −66 dBFS by >60 dB and passes −23 dBFS untouched; holds through 100 ms pauses; no chatter in the hysteresis band; smooth open/close; limiter keeps peaks ≤ −0.5 dBFS at +12 dB output gain and reports it |
| Unit tests (presets) | Every preset valid and recognized after applying; presets distinct; manual change → Custom; gains untouched |
| Hardware (`levels_check`, real pipeline, vs neutral control) | Input +6 → +6.0 dB; output −10 → −9.8 dB; +12/−6 → +6.0 dB; gate above room → −107.5 dB; gate at −90 dB → 0.0 dB; 0 dropouts |
| Real app | Preset chips show Custom for the user's DeepFilterNet 100 %; Balanced applied DeepFilterNet 80 %; setting 100 % again switched back to Custom; Audio Levels section shown; user's settings restored |
| clippy `-D warnings`, rustfmt, tests, build | Clean; 49 Rust tests + 1 hardware test; 15 frontend tests; build OK |

## Milestone 5 verification (2026-11-14)

| Check | Result |
|---|---|
| License | Upstream README read on GitHub: code **and** model weights MIT OR Apache-2.0 (commit `7b14b89`); no usage restrictions |
| Implementation spike | dfn3_rs: 0.20 ms/frame, worst 4.1 ms, 0 allocations, output within −57.5 dB of official. Tract port: 206 allocations/frame, 202 ms worst frame (rejected) |
| Unit tests (engine) | Exact delayed bypass; strength 0 untouched; delay 1440 samples; removes fan noise; higher strength removes more; strength mapping; watchdog trips only on sustained overload; overloaded engine passes audio through; real time in development builds |
| Clean-reference comparison (2 TTS voices + fan + clicks) | Speech level within 1 dB for both. DeepFilterNet SI-SDR +3 to +6 dB over the noisy input, RNNoise +0 to +1 dB |
| Live mic, both engines | Both bypasses 0.00 dB; 0 dropouts; 0.13 / 0.57 ms per frame. Live speech figures not reliable (too little speech) |
| Real app | Switching RNNoise ↔ DeepFilterNet restarts audio with the new engine (log confirmed), status Active; off = "Bypassed" at 0.05 ms/frame; 0 dropouts; user's settings restored (RNNoise, 100 %) |
| clippy `-D warnings`, rustfmt, tests | Clean; 39 Rust tests + 1 hardware test; 14 frontend tests |

## Milestone 4 verification (2026-11-14, Realtek mic → RNNoise → VB-CABLE)

| Check | Result |
|---|---|
| Unit tests (engine) | Bypass bit-exact (one-frame delay); RNNoise delay measured = 480 samples; strength is an exact wet/dry mix; on/off is an exact one-frame linear crossfade; 16-bit scaling reaches RNNoise; output always finite and bounded |
| Real audio (`denoise_check`, user speaking with pauses + background noise) | **24.3 dB noise removed** where no voice is detected; **speech −0.7 dB** median (−1.7 dB p25); bypass 0.00 dB |
| Engine CPU | Release: 0.13–0.14 ms per 10 ms frame (~1.4 % of a core). Development build: ~2.5 ms |
| Release soak with RNNoise, 180 s, while two other projects compiled | 2 dropouts, both during system-wide stalls (WASAPI glitch flags; mic silent 30–42 ms); otherwise margin steady ~19 ms |
| Development build in the real app, same load | 5 dropouts + 1 resync in ~2.5 min with **no** WASAPI glitches: the unoptimized build is too slow under heavy CPU load. Use `--release` for listening tests |
| Real app | Engine status "Active" with switch on; strength slider and engine choice shown; telemetry live |
| clippy `-D warnings`, rustfmt, unit tests, `tsc`, Vitest, `npm run build` | Clean; 30 Rust tests + 1 hardware test; 12 frontend tests; build OK |

## Milestone 3 verification (2026-11-13, Windows 10 22H2, Realtek mic → VB-CABLE 48 kHz)

| Check | Result |
|---|---|
| Routing proof: mic vs CABLE Output cross-correlation (`route_check`) | 0.983 / 0.997 / 0.967 in three runs; control ≈ 0; identical levels |
| End-to-end latency | 124–127 ms, of which VB-CABLE alone is 80.6 ms (`cable_latency`, correlation 1.0000) |
| Final build, 5-min soak: idle phase (0–150 s) | 1 underrun, during a system-wide stall (both devices silent ~40 ms, WASAPI glitch flag); otherwise 0. Drift settled at about +25 ppm, steady within ±10 ppm |
| Final build, 5-min soak: 8 busy CPU threads (150–300 s) | 1 underrun, during a system-wide stall (mic silent 89 ms, WASAPI glitch flags). No resyncs, no errors |
| Real app | Starts routing automatically; banner shows "Virtual mic live · … 32 ms buffer"; Settings shows formats, buffer, drift and counters; switch off → "Virtual mic off" and pipeline stopped; on → restarted (log confirms) |
| Real app (user) | Discord with microphone = CABLE Output: mic test works. The first attempt failed because Discord's *speaker* was also set to the cable; the fix is documented and now detected |
| Conflict warning | `cable_occupy` playing silence into CABLE Input → banner/Settings named it within 2 s and cleared when it stopped. Occupying CABLE In 16ch while Shuusshy runs fails with `DeviceBusy`, confirming VB-CABLE's shared-cable lock |
| Dropouts during the user's session | 4 dropouts / 2 device glitches early in the session (while Discord was looping into the cable); none added over the following several minutes |
| Unit tests | 22 passed (incl. drift simulations of measured timing, jitter and stalls) + 1 hardware test |
| clippy `-D warnings`, rustfmt, `tsc`, Vitest (11), `npm run build` | Clean / passed (re-run on the final code) |

Iterations: two drift-control designs failed the hardware soak (±1700 ppm swings; then 16
underruns in 5 minutes) before the final margin-based design. Details are in
docs/AUDIO_ENGINE.md.

## Not yet verified

- **The final build's routing correlation.** Three attempts on the final build (one soak, two
  with the user speaking) all saw the Realtek mic deliver almost nothing (−91 dBFS), so they
  proved stability, not routing. `mic_level` shows Windows reports the mic as unmuted at 89%,
  yet both raw channels read about −89 dBFS RMS (−68 dBFS peak). This morning it read −23 dBFS.
  The cause is upstream of Shuusshy: a hardware mute key, the manufacturer audio console, or a
  driver setting. The signal path is unchanged since the three verified runs. To check: fix the
  mic, confirm with `mic_level`, then run `route_check` while speaking.
- Teams / Zoom (Discord verified by the user).
- **Stream recovery on actual device removal** (mic unplugged mid-stream). The code paths
  exist, but hot-plug could not be exercised (see Milestone 2).
- Microphones at rates other than 48 kHz (the input resampler path), USB/Bluetooth mics.
- Long soak (hours).

## Known limitations / issues

- Mic rename works (used by the user). A VB-CABLE update may reset the name; the button
  offers it again. Apps may need a restart to show the new name.
- **Glass window on Windows 10** uses the Blur effect. Windows 10 has no rounded window corners,
  so the glass banner is square there. Very bright wallpapers reduce contrast: lower the
  transparency.
- **License keys can't be revoked** (offline by design) and, as open source, the check can be
  removed by rebuilding. Back up `%USERPROFILE%\.shuusshy\license-signing.key`.
- The installer does not ask for the Pro key; the app asks on first run ("Have a key?").
- **Silent uninstall leaves the virtual mic named "Shuusshy Microphone"** (renaming back needs
  an admin prompt, which silent mode must not show). The interactive uninstaller offers it, and
  Settings → Audio Output can restore it before uninstalling.
- The installer and executable are **not code-signed** yet, so Windows SmartScreen may warn on
  first run (signing belongs to Milestone 10 / release).
- Upgrades of the portable ZIP are manual: replace `shuusshy.exe` and keep the `data` folder.

- Update checks show "not available in this build" until Milestone 10.

- **You hear yourself slightly late while monitoring** (mic capture + engine delay + headphone
  output buffer). The monitor delay has not been measured, because loopback capture does not work
  on the test headset; it is expected to be below the ~125–155 ms virtual-mic path, since it
  skips VB-CABLE. Normal for monitoring.
- The loudspeaker warning is name-based ("Speakers …"), so some headsets (like the Razer 7.1
  virtual device) show it although they are headphones.
- `tauri dev` restarts the app whenever anything in `src-tauri/` changes, including the
  verification tools in `examples/`, which closes an open test panel. Development only.

- **The Realtek microphone sometimes delivers only its digital noise floor (≈ −92 dBFS)** even
  though Windows shows it unmuted at 89 %. This was seen in several sessions; at other times it
  worked (−23 to −53 dBFS). It is outside Shuusshy (Windows/driver side) and still unexplained.
- **Voice isolation cannot separate voices talking at the same time**, and only partly
  suppresses a background talker less than ~6 dB quieter than you. About 1 % of voice blocks are
  still attenuated at the default margin (mostly sentence onsets after silence). Results come
  from text-to-speech material; human listening tests are planned for Milestone 11.

- **Engine delay**: RNNoise 10 ms, DeepFilterNet 30 ms (constant, kept when off). Switching
  engines restarts audio for a moment.
- **DeepFilterNet needs a reasonably modern CPU** (AVX2 ideal). On slow PCs the watchdog falls
  back to untouched audio and asks the user to choose RNNoise. Not yet tested on such a PC.
- Speech-quality figures come from text-to-speech voices plus synthetic noise, not human
  listening tests. Human evaluation is planned for Milestone 11.
- Development builds now compile the audio libraries optimized (RNNoise 0.17 ms, DeepFilterNet
  0.47 ms per frame in `tauri dev`), so `--release` is no longer needed for realistic listening.
- Speech-quality judgement so far is objective (level kept within ~1 dB) plus the user's ear.
  No formal intelligibility metric yet (Milestone 11).

- **Latency target (~100 ms) is not met with VB-CABLE defaults:** about 125 ms total, of which
  about 81 ms is VB-CABLE's own buffering and about 45 ms is Shuusshy's.
- **System-wide audio stalls** longer than 20 ms (seen under load, flagged by WASAPI) cause one
  dropout each. Recovery is automatic.
- **Another app using VB-CABLE as its output**, or holding it in exclusive mode (observed with
  Discord's speaker set to the cable, and with "PureMic"), blocks or echoes into routing. Shuusshy
  now names the app (shared mode) or shows "cable busy" (exclusive), and retries every 2–30 s.
- **The microphone is captured whenever routing is on.** Nothing is recorded or stored; it is
  passed straight to VB-CABLE. Turn off "Send microphone to virtual mic" to release the mic.
- No system tray yet (Milestone 8). Closing the banner exits the app and stops routing.
- At startup the window is briefly always-on-top before a saved "off" setting is applied.
- No LICENSE file yet; no git commits or remote yet (git identity and GitHub URL needed).
- CI workflow has not run on GitHub yet.

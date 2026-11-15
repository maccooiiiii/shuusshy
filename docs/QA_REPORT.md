# Milestone 11 — Quality assurance report

_Date: 2026-11-14. Test PC: Windows 10 22H2 (19045), Intel Core i7-7700HQ (4 cores / 8
threads), Realtek built-in microphone, VB-CABLE, Razer 7.1 virtual headset._

Every result here was measured on this PC; nothing is estimated. "Needs a person" items are
listed separately at the end, with what remains to be done.

## Summary

| Area | Result |
|---|---|
| Noise suppression and speech quality | ✅ DeepFilterNet: SI-SDR +2.9 to +5.9 dB over the noisy input, noise −21 dB, speech level −0.5 to −0.7 dB. RNNoise: noise −13 dB (80 %) / −25 to −27 dB (100 %), speech −0.3 to −0.5 dB |
| Voice isolation | ✅ At the default 6 dB margin: background talker −22.7 dB in pauses, 1.3 % of voice blocks cut by > 6 dB |
| Stability / dropouts (10-minute soak) | ✅ 0 errors, no resyncs, buffer 28–48 ms, drift controlled. 3 underruns and 6 device glitches, all in the first 5 minutes while the PC was busy building; none in the last 5 minutes |
| Latency | ✅ 123.9 ms end to end while the user spoke (Milestone 3: 125 ms, VB-CABLE about 80 ms of it). Levels equal on both ends (−22.0 / −21.8 dBFS), no gaps on the cable side. The tool's waveform-correlation verdict was weak (0.24) because drift correction was still settling in the 6 s run (−435 ppm) |
| **Speech distortion (user listening)** | ✅ **Resolved (setup, not processing).** The user first heard clear distortion in every engine and setting, and in the raw microphone too. Cause: a loose headset plug and a hot Windows mic level upstream, plus Shuusshy's input gain at +10.5 dB **and output gain at +10 dB**, which kept the limiter squashing the voice. After reseating the headset and setting both gains to 0 dB, the user judged the raw mic clean and the processed voice **acceptable** (see finding 4) |
| CPU (release build) | ✅ Audio: RNNoise 1.0 %, DeepFilterNet + isolation 3.2 %, bypass 0.3 % of one core. Interface: **fixed in this milestone** (see finding 1); now about 5–7 % of one core while the emblem animates, 0.1 % when off, about 0.7 % hidden in the tray |
| Memory | ✅ About 160–205 MB private in total, of which the backend is 7–9 MB and WebView2 (interface) is the rest. Stable over the measurements (no growth) |
| Two apps on the virtual mic | ✅ Two simultaneous shared-mode readers opened "Shuusshy Microphone" without errors. Audible content with two real apps: needs a person |
| Monitoring feedback prevention | ✅ (code-verified) VB-CABLE endpoints are excluded from "Listen on", and `start_monitor` refuses them even if asked directly |
| Updates | ✅ End to end against a local test server: a tampered signature was **rejected** ("Nothing was installed"); the genuine signed 0.1.1 was downloaded, verified and installed, and the app restarted as 0.1.1 with its settings (compact mode, RNNoise, Pro) intact |
| Installer, upgrade, uninstall, portable | ✅ Milestone 9 results (per-user install, upgrade keeps settings / Pro / start-up entry, clean uninstall, portable writes only beside the exe), repeated in the update test |
| Expiry notices and expired behaviour | ✅ Milestone 10 results: the 7-day warning and the expired card at simulated dates; expired → pipeline stopped, nothing played into the cable, settings export works |
| Configuration preservation | ✅ Across upgrade, update and reinstall (settings in %APPDATA%, Pro key, start-up entry) |
| Automated tests | ✅ 78 Rust + 24 frontend tests, clippy and rustfmt clean |

## Findings fixed during QA

1. **The interface used ~38 % of a CPU core while idle (critical, fixed).**
   - Measured per animation in the transparent glass window:
     - the pulsing status dot (animated box-shadow) cost ~23 %;
     - the emblem animation cost ~14 % (CSS background steps) and still ~19 % as a 60 fps
       compositor animation;
     - the drifting background lights (large blurred shapes) cost ~14 %.
   - Every frame such a window produces is expensive, so the fix minimises *frames*, not just
     paint:
     - the emblem is stepped by a timer at exactly 18 frames a second;
     - the status dot glows statically;
     - the background light is static.
   - Result: ~5–7 % of one core while live, 0.1 % when off. The look is unchanged apart from
     the dot pulse and the drift.
2. **The emblem kept animating while hidden in the tray (fixed).**
   - WebView2 does not tell the page that the window is hidden (`document.hidden` stays false).
   - The backend now sends a `banner-visible` event on hide, show, minimise and restore, and the
     emblem pauses.
   - Hidden-in-tray interface CPU went from 6.7 % to 0.7 %.
3. **Release-workflow notes extraction** (found in Milestone 10's dry run, fixed there).
4. **Distorted speech: traced to the setup and resolved.**
   - With the user's consent, 10-second voice recordings were made of the raw mic and the
     virtual mic at the same time (`tools/qa-record`). They were replayed through the real
     chain in six configurations (`examples/voice_variants`).
   - The user judged **every processed variant distorted, and the raw recording too**.
   - The noise suppressors treat the damaged signal as noise and chop and muffle it further.
   - Not the cause: the stereo channels are in phase (correlation +0.94 to +0.98); the
     limiter acted rarely at 0 dB input gain.
   - Contributing: Windows has the mic at 89 % (+18 dB) and Shuusshy's input gain was
     +10.5 dB. The processed level was then about 11 dB above the mic, with the limiter busy.
   - Likely causes, to check in this order: Windows/Realtek **audio enhancements** (their own
     noise suppression or echo cancelling sounds choppy and metallic); an **over-driven mic
     level/boost**; a **loose or faulty headset plug/cable**. The headset was found unplugged
     once during QA, which also explains the intermittent −91 dBFS silence seen since
     Milestone 3.
   - **Resolution:**
     - the headset was pushed in firmly, and Windows mic settings were adjusted by the user;
     - Shuusshy's **output gain was found at +10 dB** (the settings already warned "The
       safety limiter is reducing peaks");
     - input and output gain were set to 0 dB.
   - Take 7, with both gains at 0 dB: the user judged the raw mic **clean** and the processed
     voice **acceptable**.
   - Lessons for users (README / troubleshooting): keep both gains near 0 dB, and lower the
     Windows mic level rather than adding Shuusshy gain.
   - **Product follow-ups worth considering:** warn when the input is overdriven, and detect
     enabled Windows enhancements on the selected mic.
6. **Limiter warning never cleared (fixed).** The "limiter is reducing peaks" count added up
   since start-up, so the warning stayed after the gain had been fixed. A gain change now
   starts a fresh count (unit-tested).
5. **Limiter overshoot (fixed).** The −0.5 dBFS limiter was followed by the clock-drift
   resampler, whose interpolation let peaks reach −0.1 dBFS on the virtual mic. The ceiling is
   now −1.0 dBFS; measured peak −0.4 dBFS, so the output stays below full scale.

## Method

- **Quality:**
  - `examples/engine_compare` (two 22-second speech clips with noise at −32.5 dBFS);
  - `examples/isolation_bench` (target speech plus a background talker at −6/−9 dB, voice
    loudness swing ±5 dB). SI-SDR, speech-level change and noise level in pauses.
- **Stability:** `examples/route_check 6 600`: the real pipeline from the Realtek mic to
  VB-CABLE, with statistics every 10 s for 10 minutes.
- **CPU and memory:** `scripts`-style PowerShell sampling of `shuusshy.exe` and its WebView2
  child processes, once a second for 30–60 s, per state. CPU is given as % of one core (the PC
  has 8 threads).
- **Updates:**
  - Two QA builds with a loopback-only update address (`SHUUSSHY_UPDATE_ENDPOINT`, accepted only
    for 127.0.0.1/localhost) and the updater's insecure-transport switch.
  - 0.1.1 signed with the real updater key and served from 127.0.0.1:8765.
  - Official builds set neither; the clean rebuild was checked to contain no test address.

## Needs a person (not yet done)

| Check | How |
|---|---|
| Device disconnection / reconnection | ✅ partly: the user re-plugged the headset while Shuusshy ran; it noticed the device changes (4 ↔ 5 outputs) and kept the virtual mic routing without errors. Unplugging the *selected* microphone itself was not tested |
| Device switching | Select a second microphone (only one physical mic exists on this PC, so not testable here) |
| Two communication apps at once | ✅ The user: Discord and a second app both heard them at the same time |
| Speech intelligibility / distortion by ear | ✅ Acceptable after fixing the setup (finding 4) |
| Tray icon animation in the notification area | ✅ The user: animating in the accent colour |
| Clean PC without WebView2 | Install on a PC or VM without WebView2 to check the bootstrapper download |

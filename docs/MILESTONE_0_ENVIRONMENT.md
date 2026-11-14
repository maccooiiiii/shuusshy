# Shuusshy — Milestone 0: Environment Inspection Report

**Date:** 2026-11-13
**Inspected by:** Claude Code (read-only inspection; no project files created, nothing installed intentionally — see note in §2)

---

## 1. System

| Item | Finding |
|---|---|
| OS | Windows 10 Pro 22H2 (10.0.19045), 64-bit |
| CPU | Intel Core i7-7700HQ, 4 cores / 8 threads (2017 mobile part — a good "modest hardware" baseline for DeepFilterNet CPU testing) |
| RAM | 23.9 GB |
| Free disk (C:) | ~369 GB |
| Package manager | winget available |

## 2. Toolchain

| Tool | Status | Notes |
|---|---|---|
| Rust / Cargo / rustup | **MISSING** | Not on PATH, no `~/.cargo` or `~/.rustup`. **Blocker for Milestone 1.** |
| MSVC C++ build tools | ✅ Present | Visual Studio Professional 2026 with VC x86/x64 tools. (`cl`/`link` not on PATH is normal outside a dev shell; Rust's MSVC target finds them via vswhere.) |
| Windows SDK | ✅ 10.0.26100.0 | |
| WebView2 Runtime | ✅ 157.0.4341.46 (machine-wide) | |
| Node.js | ✅ v26.13.1 | Supported by Vite / Tauri CLI |
| npm | ✅ 11.20.0 | pnpm/yarn not installed (not needed) |
| Git | ✅ 2.56.1 | **Global `user.name` / `user.email` not set** — commits will fail until configured. |
| GitHub CLI (`gh`) | Not installed | Optional; useful for creating the repo and releases. |
| CMake / Clang | Not installed | Not required for the recommended crates (see §5). Only needed if we fall back to building C RNNoise or certain ONNX runtimes. |
| Python | Now present (3.15.2) | ⚠️ Running `python --version` caused the Windows Python install manager to auto-download and install Python. This was unintended side-effect of the inspection. Not needed by Shuusshy; you may leave or remove it. |

## 3. Repository

- Working folder `C:\Users\Ain\Documents\#KWAYET` is **empty** and **not a git repository**. Nothing to preserve or overwrite.
- ⚠️ **The path contains `#`.** Vite (and several Node/URL-based tools) treat `#` as a URL fragment and fail to resolve modules under such paths. **Recommendation:** create the project at a path without `#` or spaces, e.g. `C:\dev\shuusshy`.
- No GitHub username or remote URL known yet — required before configuring `origin` (Part 12.1).

## 4. Audio Devices (as reported by Windows)

**Capture (recording) endpoints — active**
- `Microphone (Realtek High Definition Audio)` — physical mic (likely laptop internal / jack)
- `CABLE Output (VB-Audio Virtual Cable)` — **the virtual microphone endpoint apps will select**

**Render (playback) endpoints — active**
- `Speakers (Realtek High Definition Audio)`
- `Speakers (7.1 Surround Sound)` — Razer device (likely a headset → suitable for monitoring tests)
- `Intel Display Audio HDMI 1`
- `CABLE Input (VB-Audio Virtual Cable)` — **where Shuusshy will write processed audio**
- `CABLE In 16ch (VB-Audio Virtual Cable)` — 16-channel variant; Shuusshy must pick the 2ch `CABLE Input`, not this one

**Notable findings**
- ✅ **VB-CABLE is installed and its endpoints are functional.**
- ⚠️ A second `VB-Audio Virtual Cable` driver instance reports **Error code 10 (device cannot start)**. The working instance is OK, so this is probably a leftover from a reinstall. Not a blocker. Cleanup (Device Manager → remove the failed instance) is your call. I will not touch drivers.
- ℹ️ Leftover **Krisp Audio** endpoints exist (not present/active). Shuusshy's device list should filter or clearly label non-present devices. If Krisp is ever running at the same time, the audio would be processed twice.
- Inactive/unplugged: a second "High Definition Audio Device" mic/headphones, Stereo Mix (disabled).

**Not yet verified (requires running code in Milestone 2/3):** each device's native sample rate, channel count, and WASAPI shared-mode mix format.

## 5. Technical Assessment & Recommended Choices

| Area | Recommendation | Rationale / Risk |
|---|---|---|
| Audio I/O | `cpal` (WASAPI backend) | Mature and pure Rust. Device add/remove notifications are limited, so hot-plug detection may need polling or a small `windows-rs` `IMMNotificationClient` shim. |
| RNNoise | `nnnoiseless` crate (pure-Rust port of RNNoise, BSD-3-Clause) | No C toolchain needed. Fixed **48 kHz, 480-sample (10 ms) mono frames**, samples scaled to the i16 range. **RNNoise has no native strength parameter.** "Strength" must be a documented wet/dry mix or gain floor. It also outputs a VAD probability, which is useful for the noise gate. |
| DeepFilterNet | `deep_filter` crate (DeepFilterNet3, `tract` inference, MIT/Apache-2.0) | Also **48 kHz, 480-sample hop**. Exposes `atten_lim_db`, a genuine suppression-strength control. **Risks to verify in M5:** the crates.io version may lag the repo, so it may need a git-pinned dependency; the model-weight license must be confirmed; real-time CPU use on the i7-7700HQ must be measured. |
| Resampling | `rubato` | The mic and VB-CABLE may not run at 48 kHz, so conversion is needed at both ends. |
| **Clock drift** | Adaptive ring buffer with drift compensation between the mic and VB-CABLE streams | ⚠️ **Key engineering risk.** The two devices run on independent clocks. Without compensation, the buffer slowly under- or over-runs, causing clicks or growing latency. This must be solved in M3 before any noise suppression is added. |
| Real-time safety | Lock-free SPSC ring buffers (`rtrb`), pre-allocated frames, atomics for parameters, meters published via atomics/bounded channel | Matches Part 7.2 |
| Settings | `rusqlite` with `bundled` feature (compiles SQLite with MSVC — available) + `serde_json` export/import | |
| UI | Tauri 2 + React + TS + Vite + Tailwind | No blockers found |
| Updater | `tauri-plugin-updater`, signing key generated locally and stored **outside** the repo, put into GitHub Actions secrets | You will need to generate and safeguard the key yourself |

### Policy items needing your decision (not blockers for M1)
1. **License vs. two-year expiration (Part 10.5).** Under MIT or Apache-2.0, anyone can rebuild without the expiry check. The expiration can only apply to *official binaries* as a transparent, documented build feature. If you want stronger binary terms, that needs a separate binary EULA reviewed by a lawyer. I recommend **Apache-2.0 or MIT** for source, with the expiry clearly disclosed. I'll draft the documentation, not legal terms.
2. **VB-CABLE** is donationware by VB-Audio. Plan: detection plus a link to the official download, with no bundling (per spec).

## 6. Missing Prerequisites (action list)

| # | Item | Required for | Suggested command / action |
|---|---|---|---|
| 1 | **Rust toolchain (stable, `x86_64-pc-windows-msvc`)** | M1 | `winget install Rustlang.Rustup`, then `rustup default stable` |
| 2 | **Git identity** | First commit | `git config --global user.name "…"` / `user.email "…"` |
| 3 | **Project location without `#`** | M1 | e.g. `C:\dev\shuusshy` |
| 4 | GitHub username / repo URL | Remote setup (can wait until end of M1) | Create an empty public repo `shuusshy`, or install `gh` (`winget install GitHub.cli`) |
| 5 | Headphones for monitoring tests | M7 (and M3 listening checks) | Razer 7.1 device appears suitable; please confirm |
| 6 | (Optional) Remove failed VB-Audio driver instance | Hygiene | Device Manager, your decision |

## 7. Proposed Implementation Sequence

Follows Part 15, with two adjustments driven by this inspection:

- **M1 — Desktop foundation:** Tauri 2 + React/TS/Vite/Tailwind scaffold, frameless always-on-top compact banner (~360 px), repo skeleton and docs stubs, `PROJECT_STATUS.md`, restrictive Tauri capabilities, settings module skeleton (SQLite), basic CI (fmt/clippy/tsc/tests on `windows-latest`).
- **M2 — Device management:** cpal enumeration, separating physical mics from virtual endpoints (VB-CABLE identified by name and filtered, `CABLE In 16ch` excluded, Krisp leftovers labelled), selection persisted, hot-plug via `IMMNotificationClient`. Tested against the Realtek mic + VB-CABLE on this machine.
- **M3 — Pass-through + drift handling:** mic → ring buffer → resample → `CABLE Input`; verified by recording from `CABLE Output` in a real app (e.g. Windows Sound Recorder / browser). **Adjustment:** drift compensation and latency measurement are part of the M3 exit criteria.
- **M4 — RNNoise** (`nnnoiseless`), 480-frame pipeline, true bypass, engine status state machine (initializing / processing / bypassed / error).
- **M5 — DeepFilterNet**: first a short **spike** to confirm build, license and real-time CPU on this i7-7700HQ. If it can't run in real time, report it honestly before going further.
- **M6 — Controls & presets** mapped to real parameters (RNNoise mix, DFN `atten_lim_db`, gains, VAD/RMS gate).
- **M7 — Live test & meters**: pre/post-suppression RMS/peak taps, monitor output to a chosen headphone device, feedback warning.
- **M8 – M12** as specified (UI polish/tray → installer/portable → CI/updater/expiry → QA → 1.0.0).

Each milestone ends with the Part 17 report format and waits for your review.

---

**Milestone:** 0 — Environment Inspection
**Status:** Complete (inspection only)
**Completed:** OS/hardware, toolchain, WebView2, MSVC, repository state, audio endpoints, VB-CABLE detection, technical risk assessment
**Files Created or Modified:** `SHUUSSHY_M0_ENVIRONMENT_REPORT.md` (this file). Side effect: Python 3.15.2 auto-installed by Windows' Python launcher during version probing.
**Features Implemented:** None (by design)
**Tests Performed:** Command-line inspection only. No audio was captured or played.
**Build Result:** N/A
**Known Issues:** Rust missing; git identity unset; `#` in the current path is incompatible with Vite; one VB-Audio driver instance in error state (non-blocking)
**Next Milestone:** M1 — Desktop Foundation (after prerequisites 1–3)

# Shuusshy 1.0.0: release acceptance (PRD Part 16)

Each criterion with the evidence it rests on. "QA" refers to [QA_REPORT.md](QA_REPORT.md);
"M" to the milestone verification sections in [PROJECT_STATUS.md](../PROJECT_STATUS.md).

## Audio

| Criterion | Status | Evidence |
|---|---|---|
| Physical microphone capture works | ✅ | M2–M3; QA recordings of the Realtek mic |
| Virtual microphone routing works | ✅ | M3 correlation 0.97–0.997; QA: speech levels equal on mic and virtual mic (−22.0 / −21.8 dBFS), 124 ms latency; Discord and a second app heard the user |
| RNNoise works | ✅ | M4 on the real mic (−24 dB noise); QA benchmark: noise −13 to −27 dB, speech within 0.5 dB |
| DeepFilterNet works | ✅ | M5; QA benchmark: SI-SDR +3 to +6 dB, noise −21 dB |
| Engine switching works | ✅ | M5; QA (release build switched RNNoise ↔ DeepFilterNet live) |
| Bypass works | ✅ | M4 exact-bypass unit tests; QA release CPU in bypass 0.3 % |
| Audio remains stable | ✅ | QA 10-minute soak: 0 errors, no resyncs, no dropouts in the last 5 minutes |
| No unacceptable speech distortion | ✅ | QA finding 4: after fixing the setup (headset plug, gains at 0 dB), the user judged the processed voice acceptable |

## Testing

| Criterion | Status | Evidence |
|---|---|---|
| Original monitoring works | ✅ | M7: the user heard the original |
| Processed monitoring works | ✅ | M7: the user heard the processed signal; QA listening test |
| Comparison works | ✅ | Original/processed switch is time-aligned (unit test) |
| Real input meter works | ✅ | M7 unit tests (exact sine levels) and the real app (−54 dB room) |
| Real processed meter works | ✅ | M7 (output meter follows gain; "—" after suppression) |
| Monitoring can be stopped immediately | ✅ | M7: Stop/Esc/closing the panel; hiding to the tray also stops it |
| No audio is saved by the test feature | ✅ | The monitor writes only to a ring buffer to the headphones; no file APIs in the path |

## Interface

| Criterion | Status | Evidence |
|---|---|---|
| Banner is compact | ✅ | 360 px wide, compact and expanded modes (M8) |
| Main controls immediately accessible | ✅ | On/off, Test, engine and strength on the banner |
| Settings remain simple | ✅ | Sections with advanced items folded away |
| System tray works | ✅ | M8 (user confirmed menu); QA (animated icon confirmed) |
| All indicators reflect actual state | ✅ | Emblem/tray/status follow the engine state, not the switch (M8, unit tests) |

## Security and privacy

| Criterion | Status | Evidence |
|---|---|---|
| Audio processed locally | ✅ | No network code in the audio path; 1.0.0 has no update endpoint at all |
| No mandatory account | ✅ | None exists |
| No microphone recording by default | ✅ | Nothing writes audio to disk (QA tools record only when run on purpose) |
| No audio uploads | ✅ | Shuusshy's code makes no connections in 1.0.0. WebView2 (Microsoft) opens two HTTPS connections to Microsoft on start-up even with background networking disabled; audio never enters WebView2. Disclosed in PRIVACY.md, the terms and About |
| Tauri permissions restricted | ✅ | Capabilities: window drag, minimise, close and events only; commands validated in Rust; fixed URLs only |

## Distribution

| Criterion | Status | Evidence |
|---|---|---|
| Installer works | ✅ | M9 (per-user install, shortcuts, Apps entry); final 1.0.0 installer: see below |
| Portable build works | ✅ | M9 (data beside the exe, no AppData writes) |
| Updates preserve settings | ✅ | M9 upgrade 0.1.0 → 0.1.1; QA in-app update 0.1.0 → 0.1.1. 1.0.0 itself has no update channel; installing a newer version over it keeps settings (same mechanism as M9) |
| Release artifacts are verified | ✅ | SHA256SUMS.txt for both artifacts; final installer installed and run (below) |
| Expiration metadata is valid | ✅ | Signed release metadata verified by the app: see below |
| Expired versions stop processing safely | ✅ | M10: at the simulated expiry date the pipeline stopped and nothing played into the cable; settings and export kept |

## Final 1.0.0 artifacts

Built locally on 2026-11-14 in `release/1.0.0/` (not committed):

| File | SHA-256 |
|---|---|
| `Shuusshy_1.0.0_x64-setup.exe` | `db43d3662c13e10fc5b847b109f43c7a84d63867d08df02ac54f934df308f10c` |
| `Shuusshy_1.0.0_x64_portable.zip` | `18c241fa1078478b651b1549e1f5d8c8016cf312434fc8c9a4f1e74626d2ecc7` |

Verification of these exact files:
- The installer matched SHA256SUMS.txt and installed silently per-user (exit 0); the version is
  1.0.0.
- Settings → Application and Updates shows **"Official stable release of 2026-11-14, valid until
  2028-11-14 (730 days left)"**, so the signed metadata verifies with the release public key.
- Settings, Pro key and accent were kept from the previous install.
- Audio: DeepFilterNet active, and Shuusshy's session on CABLE Input is **ACTIVE**.
- The test-server address is absent from the binary; there is no update endpoint.
- Network:
  - Shuusshy's code opens no connections.
  - The WebView2 runtime still opens two HTTPS connections to Microsoft at start-up, even with
    background networking, component updates, SmartScreen, sync and pings disabled.
  - This is disclosed in About, PRIVACY.md, the installer terms and KNOWN_LIMITATIONS.md.
    The release notes previously claimed "no network connections at all"; that was corrected
    before this build.

**Decision: 1.0.0 meets the Part 16 acceptance criteria above and is declared stable**, with the
limitations in [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md).

## Not part of 1.0.0

- **GitHub release:** no repository exists yet. Artifacts are built locally; publishing is
  deferred. The release workflow is ready for future versions.
- **Automatic updates:** not available in 1.0.0 (no repository address baked in).

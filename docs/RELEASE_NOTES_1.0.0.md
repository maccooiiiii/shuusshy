# Shuusshy 1.0.0

_Released 2026-11-14 · stable · valid until 2028-11-14_

**Less Noise. More Voice.** Shuusshy removes background noise from your microphone on your PC
and passes your clean voice to Teams, Discord, Zoom and any other app through a virtual
microphone.

## What you get

- **Two noise-cancellation engines.**
  - **RNNoise** is free and very light: about 1 % of one CPU core, 10 ms delay.
  - **DeepFilterNet** (Pro) is the strongest: about 3 % of one core, 30 ms delay.
  - Both have an adjustable strength and a genuine off switch.
- **Voice isolation (Pro).** Mutes other people talking more quietly than you while you pause.
- **Live test.** Hear your mic before and after processing on your headphones, with real level
  meters. Nothing is recorded.
- **Virtual microphone.** Through VB-CABLE (free, installed separately), optionally shown as
  "Shuusshy Microphone" in your apps.
- **A small floating banner.**
  - Compact or expanded view; glass look with seven accent colours or your Windows accent.
  - An animated tray icon while noise cancellation is working.
  - Start with Windows: Shuusshy starts quietly in the tray.
- **Presets:** Balanced, Strong Noise Reduction, Speech Priority, Low CPU Usage.
- **Settings backup:** export and import as a file.
- **Private by design.** Everything runs on your PC. No account, no recordings, no uploads.
  Shuusshy itself makes no network connections in this release; Windows' WebView2 component
  that draws the window may contact Microsoft on its own (see PRIVACY.md).

## Install

- `Shuusshy_1.0.0_x64-setup.exe`: installs for your Windows account (no administrator rights).
- `Shuusshy_1.0.0_x64_portable.zip`: run from any folder; settings stay beside the exe.
- Check your download against `SHA256SUMS.txt`.

You also need **VB-CABLE** (once): <https://vb-audio.com/Cable/>. Shuusshy shows the steps.
See [INSTALLATION.md](../INSTALLATION.md).

## Good to know

- **Validity.** This release is valid for two years, until **2028-11-14**. You get a reminder 30
  days before and a warning 7 days before. After that date, noise cancellation and the virtual
  mic stop until you install a newer version; your settings are kept.
- **Updates.** This release has **no automatic update channel**. Install newer versions from
  where you downloaded Shuusshy; installing over the old version keeps your settings.
- **Gain.** For the cleanest sound, keep Shuusshy's input and output gain near 0 dB and set the
  microphone level in Windows instead (see TROUBLESHOOTING.md).
- **SmartScreen.** The installer is not code-signed yet, so Windows SmartScreen may warn: choose
  "More info → Run anyway" if you trust the source.

Full list: [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md).

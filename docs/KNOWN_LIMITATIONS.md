# Known limitations: Shuusshy 1.0.0

These are known and accepted for 1.0.0. They are documented so that nothing is hidden.

## Audio

- **Latency is about 125 ms** from microphone to app (measured 124 ms). About 80 ms of it is
  VB-CABLE's own buffering; Shuusshy's part is about 45 ms. Engine delay: RNNoise 10 ms,
  DeepFilterNet 30 ms.
- **The input matters.** A distorted or overdriven microphone stays distorted after noise
  suppression, and gets choppier: noise suppressors treat damage as noise. Keep the Windows mic
  level moderate, Windows "audio enhancements" off, and Shuusshy's gains near 0 dB.
- **Voice isolation cannot separate voices that talk at the same time.** It only partly
  suppresses a background talker who is less than about 6 dB quieter than you.
- **System-wide audio stalls** (heavy CPU load) can cause a short dropout; Shuusshy recovers by
  itself.
- **Another app playing into VB-CABLE** (for example a call app whose *speaker* is set to
  "CABLE Input") blocks or echoes into the virtual mic. Shuusshy names the app and explains it.
- **DeepFilterNet needs a reasonably modern CPU.** On a slow PC, Shuusshy detects overload,
  passes audio through untouched and suggests RNNoise. Not yet tested on such a PC.
- While routing is on, the microphone is in use (nothing is recorded). Turn off "Send
  microphone to virtual mic" to release it.

## Testing and monitoring

- You hear yourself slightly late while monitoring (capture + engine + headphone buffers).
- The loudspeaker warning is name-based, so some headsets named "Speakers…" show it.

## Interface

- **Glass window on Windows 10** uses the Blur effect with square corners (Windows 11 rounds
  them). Very bright wallpapers reduce contrast: lower the transparency.
- The interface (WebView2) uses about 160–200 MB of memory, and about 5–7 % of one CPU core
  while the emblem animates (0.1 % when noise cancellation is off, under 1 % in the tray).

## Privacy

- **WebView2 may contact Microsoft.** The window is drawn by Microsoft Edge WebView2. Even with
  background networking, component updates, SmartScreen, sync and pings turned off, it opened
  two HTTPS connections to Microsoft servers on the test PC. They carry no audio or Shuusshy
  data. Shuusshy's own code makes no connections in 1.0.0.

## Distribution, updates, licensing

- **No automatic update channel in 1.0.0.** Install newer versions manually; settings are kept.
- **Not Authenticode code-signed:** Windows SmartScreen may warn on first run.
- **The validity period uses the PC clock** and is best-effort by design. So are the offline
  Pro keys, which cannot be revoked. Shuusshy uses no anti-tampering.
- Portable edition: upgrade by replacing `shuusshy.exe` and keeping the `data` folder.
- **Silent uninstall** leaves the virtual mic named "Shuusshy Microphone". The interactive
  uninstaller offers to rename it back, and so does Settings → Audio Output.
- A VB-CABLE update can reset the "Shuusshy Microphone" name; the button offers it again.

## Not tested on the development PC

- Unplugging the *selected* microphone while running. Re-plugging the headset was tested:
  routing continued.
- Switching between two physical microphones (only one exists on the test PC).
- Installing on a PC without WebView2 (the installer downloads it from Microsoft).
- The GitHub release workflow (no repository exists yet).

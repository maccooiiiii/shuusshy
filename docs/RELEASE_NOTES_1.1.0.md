# Shuusshy 1.1.0

_Stable · valid for two years from its release date_

## What's new

- **VB-CABLE comes with the Windows installer.** No separate download any more.
  - At the end of setup Shuusshy asks "Install VB-CABLE now?". Say Yes, click
    **Install Driver** in VB-Audio's window (Windows asks for administrator permission), and
    restart if asked.
  - Skipped it, or using the portable ZIP? Settings → Audio Output → **Install VB-CABLE**.
  - Updates never ask again, and the question is skipped when VB-CABLE is already installed.
  - VB-CABLE is included unmodified. VB-CABLE is made by VB-Audio Software
    (www.vb-cable.com). VB-CABLE is a donationware, all participations are welcome.
- **A user guide inside the app.** Press the **?** button on the banner (or Settings → About).
  "Getting started" is a live checklist that ticks itself as Shuusshy finds the virtual cable,
  your microphone and active noise cancellation, with buttons that jump to each fix.
- **macOS beta.** A universal disk image for Apple Silicon and Intel Macs is attached. It uses
  BlackHole 2ch as the virtual microphone. It is not notarised yet, and it has not been tested on
  many Macs: see the [macOS notes](https://github.com/maccooiiiii/shuusshy/blob/main/docs/MACOS.md)
  for how to open it the first time.

Audio processing, settings and Pro features are the same as in 1.0.1.

## Updating

- **Installed with the installer:** Shuusshy offers the update under Settings → Application
  and Updates → Check for updates. Your settings and Pro key are kept, and the update does not
  ask about VB-CABLE.
- **Portable edition:** download the new ZIP and replace `shuusshy.exe`, keeping the `data`
  folder. The `vbcable` folder holds VB-Audio's setup.

Check downloads against `SHA256SUMS.txt` (Windows) and `SHA256SUMS-macOS.txt` (Mac). See the
[1.0.0 release notes](https://github.com/maccooiiiii/shuusshy/blob/main/docs/RELEASE_NOTES_1.0.0.md)
for the full feature list and the
[known limitations](https://github.com/maccooiiiii/shuusshy/blob/main/docs/KNOWN_LIMITATIONS.md).

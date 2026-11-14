# Installing Shuusshy

Shuusshy runs on Windows 10 and 11 (64-bit). It needs two things:

1. **Shuusshy itself**: the installer or the portable ZIP (below).
2. **VB-CABLE**, the free virtual audio driver that carries the cleaned voice to other apps.
   Shuusshy never installs or modifies drivers itself.

## Option A: installer (recommended)

`Shuusshy_<version>_x64-setup.exe`

- Installs for **your Windows account only**, into `%LOCALAPPDATA%\Shuusshy`. **No
  administrator rights** are needed.
- Creates a Start menu shortcut. A desktop shortcut is optional: tick it on the last page.
- **Upgrades:** run a newer installer over the old one. If Shuusshy is running, the installer
  asks to close it. Your settings (`%APPDATA%\com.shuusshy.app`), your Pro key and "Start with
  Windows" are kept.
- **Uninstall:** Windows Settings → Apps → Shuusshy.
  - The uninstaller removes the program, the shortcuts and the "Start with Windows" entry.
  - If you renamed the virtual mic to "Shuusshy Microphone", it offers to rename it back to
    "CABLE Output". Windows asks for administrator permission; you can decline.
  - Settings are kept unless you tick "Delete the application data".
- The installer uses Microsoft Edge **WebView2**, which Windows 10/11 already include. If it is
  missing, the installer downloads it from Microsoft.

## Option B: portable ZIP

`Shuusshy_<version>_x64_portable.zip`

- Extract it anywhere you can write to (for example a USB drive) and run `shuusshy.exe`.
- The `shuusshy.portable` file next to the exe switches on portable mode:
  - settings, your Pro key and the WebView2 cache stay in the `data` folder beside it;
  - nothing is written to `%APPDATA%` or `%LOCALAPPDATA%`;
  - Settings → Application shows a **Portable** badge.
- The only thing outside the folder is the optional "Start with Windows" entry (Windows'
  per-user Run list). Turn it off before deleting the folder.
- To remove it, exit from the tray icon and delete the folder.

## Installing VB-CABLE (once)

1. Download it from the official site, <https://vb-audio.com/Cable/> (Settings → Audio Output
   → "Open in browser").
2. Extract the ZIP, right-click `VBCABLE_Setup_x64.exe` → **Run as administrator** →
   *Install Driver*.
3. Restart Windows if asked. Shuusshy detects the cable by itself; it does not need a restart.
4. In Teams, Discord, Zoom…, choose **CABLE Output** (or **Shuusshy Microphone**, if you
   renamed it) as the microphone.

VB-CABLE is donationware by VB-Audio Software; see their site for its licence.

## Checksums

Each release artifact comes with a SHA-256 file. To check a download:

```powershell
Get-FileHash .\Shuusshy_0.1.0_x64-setup.exe -Algorithm SHA256
```

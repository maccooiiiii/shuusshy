# Shuusshy on macOS (test build)

> **Status: untested test build.** It is built by GitHub's macOS machines (workflow
> "macOS test build") but has not yet been run on a real Mac. Expect rough edges and please
> report what works and what does not.

Shuusshy runs on macOS 11 (Big Sur) or later, on Apple Silicon and Intel Macs (one universal
app). Everything works offline: no account, no internet needed.

It needs two things:

1. **Shuusshy** (`Shuusshy_<version>_universal.dmg`).
2. **BlackHole 2ch**, a free, open-source virtual audio driver that carries your cleaned voice to
   other apps. Shuusshy never installs drivers itself.

## 1. Install BlackHole (once)

1. Download **BlackHole 2ch** from the official site: <https://existential.audio/blackhole/>
   (the 2-channel edition; the 16ch edition is not used).
2. Open the downloaded `.pkg` and follow the installer. macOS asks for your password.
3. You can copy the installer to a USB stick to install on a Mac without internet.

## 2. Install Shuusshy

1. Open the `.dmg` and drag **Shuusshy** onto **Applications**.
2. **First launch:** the app is not signed with an Apple Developer ID, so macOS blocks a normal
   double-click. Do this once:
   - **macOS 15 (Sequoia) and later:** double-click Shuusshy, click **Done** on the warning,
     then open **System Settings → Privacy & Security**, scroll down to the Shuusshy message
     and click **Open Anyway**, then confirm with your password.
   - **macOS 11–14:** in Applications, **right-click (Control-click) Shuusshy → Open**, then
     click **Open** in the dialog.
   - If macOS says the app "is damaged", the download was quarantined. In Terminal run
     `xattr -dr com.apple.quarantine /Applications/Shuusshy.app` and open it again.
3. When Shuusshy first uses the microphone, macOS asks for permission: click **Allow**. (Later:
   System Settings → Privacy & Security → Microphone.)

## 3. Use it

- In Shuusshy, pick your microphone. Shuusshy detects BlackHole by itself.
- In Teams, Discord, Zoom and other apps, choose **BlackHole 2ch** as the microphone.
- **Keep your speakers / headphones on your normal output**, not BlackHole: an app playing into
  BlackHole would echo into your virtual mic.

## Differences from Windows

| | Windows | macOS |
|---|---|---|
| Virtual cable | VB-CABLE | BlackHole 2ch |
| Name in other apps | "Shuusshy Microphone" (optional rename) | "BlackHole 2ch" (macOS cannot rename it) |
| Start at sign-in | Run entry in the registry | LaunchAgent in `~/Library/LaunchAgents` |
| Tray icon | System tray | Menu bar |
| Warning about other apps using the cable | Yes | No (macOS does not list them) |
| Updates | In-app | Not yet: download new versions manually |
| Signature | Not code-signed (SmartScreen) | Ad-hoc only (Gatekeeper, see above) |
| Validity period | Two years (official releases) | Test builds are "unofficial" and do not expire |

## Uninstall

1. Quit Shuusshy (menu bar icon → Quit Shuusshy).
2. Delete `Shuusshy.app` from Applications.
3. Optional: delete `~/Library/Application Support/com.shuusshy.app` (settings and Pro key)
   and `~/Library/LaunchAgents/com.shuusshy.app.autostart.plist` (start at login).
4. BlackHole has its own uninstaller (see its website).

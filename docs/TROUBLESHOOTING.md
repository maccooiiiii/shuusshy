# Troubleshooting

## My microphone is not listed

Shuusshy lists only **physical** microphones: built-in, USB, Bluetooth and jack devices. It hides
virtual ones such as VB-CABLE, Krisp or Voicemeeter. Selecting one of those would loop or
double-process your audio. The picker shows how many virtual inputs are hidden.

If a physical microphone is missing:

1. Check that it is enabled in **Windows Settings → System → Sound → Manage sound devices**
   (or the classic Sound control panel → Recording tab).
2. Unplugged devices disappear from the list. Plug it back in, and Shuusshy picks it up within
   about a second without a restart.
3. Click **Refresh** in the microphone list.

## "Microphone disconnected"

Your selected microphone was unplugged, disabled, or removed. Shuusshy keeps the selection and
switches back automatically when the device returns. You can also choose another microphone.

## A microphone shows a red error

Shuusshy opens each microphone briefly to read its audio format. Common causes:

| Message | Fix |
|---|---|
| in use exclusively by another application | Close the app using it, or in the device's Sound properties → Advanced, untick "Allow applications to take exclusive control" |
| Access denied – check Windows microphone privacy settings | Windows Settings → Privacy → Microphone: allow desktop apps to access the microphone |
| Windows Audio service is not running | Restart the "Windows Audio" service or reboot |

## "Virtual microphone not installed"

Shuusshy sends processed audio to other apps through **VB-CABLE**, a free virtual audio cable
from VB-Audio. Shuusshy does not bundle or install it.

1. Download it from the official site: https://vb-audio.com/Cable/
2. Extract the ZIP and run `VBCABLE_Setup_x64.exe` **as administrator**. Click *Install Driver*.
3. Restart Windows if asked.
4. Shuusshy detects it automatically.

## "VB-CABLE is incomplete"

Either **CABLE Input** (playback) or **CABLE Output** (recording) is disabled or missing. Enable
both in the Sound control panel (Playback and Recording tabs), or reinstall VB-CABLE.

Note: VB-CABLE also creates **CABLE In 16ch**. Shuusshy ignores it on purpose and uses the
2-channel **CABLE Input**.

## "… is playing into the virtual cable" / "the virtual cable is busy"

**Only the microphone side of an app should ever use VB-CABLE.** Shuusshy plays into
**CABLE Input**; apps such as Discord, Teams and Zoom should use **CABLE Output** as their
*microphone*. Their **speaker / output** must be your headphones or speakers, never a CABLE
device.

If an app's output is set to **CABLE Input** or **CABLE In 16ch**:
- its sound (other people's voices, mic-test playback, notifications) goes into the cable and
  comes back out of CABLE Output, into the microphone: echo, doubled audio, crackles;
- VB-CABLE's two playback endpoints share one cable. If the app started first, Shuusshy cannot
  open CABLE Input at all ("the virtual cable is busy").

Shuusshy checks this every 2 seconds and names the app in the banner. Fix it in that app's
audio settings. If Shuusshy still shows "busy", quit the app completely (including from the
system tray) and start it again after Shuusshy shows "Virtual mic live".

A program can also hold CABLE Input in exclusive mode (observed with "PureMic"). Then no app is
named, but the "busy" message appears. Quit that program; Shuusshy retries automatically.

## The banner disappeared

With **Keep running in the tray when closed** on (the default), closing the banner hides it
and the virtual microphone keeps working. Click the Shuusshy icon in the notification area to
bring it back. It may be under the ^ arrow. Right-click the icon for the menu, which also has
**Exit Shuusshy**.

## Testing the virtual microphone

1. Start Shuusshy and wait for **"Virtual mic live"**.
2. Easiest test: `mmsys.cpl` → Recording → **CABLE Output** → Properties → **Listen** → tick
   "Listen to this device", pick your **headphones** (not speakers: feedback). Untick it when done.
3. In an app: set its microphone to **CABLE Output** and use its mic test (Discord "Mic Test",
   Teams "Make a test call", Zoom "Test mic").

Hearing yourself through "Listen to this device" adds Windows' own delay on top, and can crackle
on some outputs (e.g. virtual surround). An app's mic test is a better judge of quality.

On PowerShell systems that block scripts, start the dev build with `npm.cmd run tauri dev`.

## Other apps hear me with a delay

Total delay is about 125 ms on the development PC. About 80 ms of that is VB-CABLE's own
buffering and about 45 ms is Shuusshy's. That is normal for calls, but noticeable if you monitor
yourself.

## Occasional short dropouts

If Windows audio itself stalls (the "device glitches" counter in Settings rises at the same
moment), every audio app glitches. Shuusshy recovers automatically. Frequent glitches usually
point to a driver issue (DPC latency).

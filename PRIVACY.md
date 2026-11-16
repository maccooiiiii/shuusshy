# Privacy

Shuusshy processes your microphone **only on your PC**.

## What Shuusshy does not do

- It does not record or store your microphone audio. The live test plays audio to your
  headphones in real time and saves nothing.
- It does not upload audio or send microphone content anywhere.
- It requires no account and collects no analytics or telemetry.

## Network connections

Shuusshy's own code connects to the internet for one feature only: **update checks**.

- What: Shuusshy downloads the release information file (`latest.json`) from the official
  GitHub repository and, if you install an update, the signed installer.
- When: at start-up (at most once a day) and when you click "Check for updates". Turn it off
  in Settings → Application and Updates.
- What is sent: an ordinary HTTPS request to GitHub. GitHub sees your IP address and a
  standard user agent, as with any download. Shuusshy sends no identifiers, settings or audio.
- If the repository address is not configured (development builds), no connection is made.

Opening the VB-CABLE or GitHub Releases pages uses your web browser, and only when you click.

**Microsoft WebView2.**
- Shuusshy's window is drawn by Microsoft Edge WebView2, a component of Windows.
- WebView2 can contact Microsoft services on its own: on the test PC, two HTTPS connections to
  Microsoft servers were open after start-up.
- Shuusshy starts WebView2 with background networking, component updates, SmartScreen, sync and
  pings turned off, which reduces this but does not remove it entirely.
- These connections carry no audio and no Shuusshy settings: the audio never enters WebView2,
  and Shuusshy sends nothing through it.

## What is stored on your PC

- Settings, including your Pro license key, in a local SQLite database:
  - installed: `%APPDATA%\com.shuusshy.app`;
  - portable: the `data` folder beside `shuusshy.exe`.
- The embedded browser's cache (WebView2): `%LOCALAPPDATA%\com.shuusshy.app`, or `data\webview`
  when portable.
- If you turn on "Start with Windows": one entry in your Windows sign-in list (HKCU Run).
- If you rename the virtual mic: the VB-CABLE recording device's display name in Windows.

Uninstalling removes the program and the sign-in entry; it keeps your settings unless you tick
"Delete the application data".

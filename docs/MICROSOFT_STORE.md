# Microsoft Store

Shuusshy goes to the Microsoft Store as an **MSIX package**. The Store signs it on upload (no
certificate to buy) and delivers updates.

## How the Store edition differs

| | Installer / portable ZIP | Microsoft Store edition |
|---|---|---|
| Updates | Built-in updater (GitHub) | Microsoft Store |
| VB-CABLE | Included, installed with your OK | Not included (Store policy 10.2.3); Settings links to vb-audio.com |
| Start at sign-in | Registry `Run` entry | The package's StartupTask (Windows Settings → Apps → Startup) |
| Rename the virtual mic | Available (administrator prompt) | Not available: the Store edition does not change another driver's settings |
| Signed by | Not code-signed yet | Microsoft (on upload) |

The app detects the package itself (`install::is_store_package`); there is one codebase.

## One-time setup (Partner Center)

1. Sign in at <https://partner.microsoft.com/dashboard> with the developer account.
2. **Apps and games → New product → MSIX or PWA app**, reserve the name **Shuusshy**.
3. Open the app → **Product management → Product identity** and copy three values into the
   GitHub repository (**Settings → Secrets and variables → Actions → Variables**):

   | Partner Center | GitHub variable |
   |---|---|
   | Package/Identity/Name | `STORE_IDENTITY_NAME` |
   | Package/Identity/Publisher (starts with `CN=`) | `STORE_PUBLISHER` |
   | Package/Properties/PublisherDisplayName | `STORE_PUBLISHER_DISPLAY_NAME` |

   They are not secret (they are inside every published package).

## Building the package

Actions → **Microsoft Store package** → **Run workflow**, on the release tag (for example
`v1.1.2`), or:

```
gh workflow run store.yml --ref v1.1.2
```

The run's artifact **Shuusshy-MicrosoftStore-<version>** holds `Shuusshy_<version>_x64.msix`
(and its SHA-256). Run on a branch, it is a test package without signed release metadata.

## Submission

Partner Center → the app → **Start submission**:

- **Pricing and availability:** Free; markets as you like.
- **Properties:** category *Utilities + tools* (subcategory *Audio*, or *Productivity*); privacy
  policy URL <https://maccooiiiii.github.io/shuusshy/privacy.html>; support contact
  shush_support@ianmusa.com; website <https://maccooiiiii.github.io/shuusshy/>.
- **Age ratings:** complete the IARC questionnaire (no violence, no user-generated content, no
  purchases in the app itself, no location; audio is processed locally and not shared).
- **Packages:** upload the `.msix`. Device family: Desktop.
- **Store listing (English):** text below; screenshots from `store-listing/screenshots/`
  (1920 × 1080); optional 1:1 box art `store-listing/box-art-1080.png`.
- **Submission options → Notes for certification:** text below.

### Store listing

**Description** (Store policy 10.2.4: the dependency must come first):

> Requires VB-CABLE, a free virtual audio driver by VB-Audio Software (www.vb-cable.com),
> installed separately. Shuusshy shows you how in Settings → Audio Output.
>
> Shuusshy removes background noise from your microphone on your own computer and gives your
> clean voice to Teams, Discord, Zoom, Google Meet, OBS and every other app.
>
> - Removes keyboards, fans, traffic and other background noise in real time.
> - Works in every app: your clean voice arrives as a microphone.
> - Private by design: everything happens on your computer. No account, nothing recorded,
>   nothing uploaded.
> - Live test: hear yourself before and after on your headphones, with real level meters.
> - Presets (Balanced, Strong Noise Reduction, Speech Priority, Low CPU Usage) and fine
>   control over strength, noise gate and gain.
> - A compact banner that stays on top during calls, a tray icon, and start at sign-in.
>
> Free with RNNoise. Shuusshy Pro adds the DeepFilterNet engine and voice isolation; Pro keys
> are checked offline.

**Short description:** Local, offline noise cancellation for your microphone in every app.

**Product features (up to 20):** Real-time background noise removal · Works in Teams, Discord,
Zoom, Meet, OBS · Processing stays on your computer · Live before-and-after test with meters ·
Presets and fine control · Compact always-on-top banner · Start at sign-in

**Keywords:** noise cancellation, noise suppression, microphone, virtual microphone, voice,
calls, RNNoise, DeepFilterNet

**Copyright:** © 2026 Ian Musa

### Notes for certification

> Shuusshy is a full-trust desktop app (runFullTrust) that removes background noise from the
> microphone and passes the cleaned audio to other apps through a virtual audio device.
>
> Dependency (policy 10.2.4): it requires VB-CABLE, a free third-party virtual audio driver by
> VB-Audio Software (https://vb-audio.com/Cable/). The package does not include or install the
> driver and does not change its settings; the app only links to VB-Audio's site, and the
> dependency is disclosed at the start of the description. Without VB-CABLE the app opens and
> shows the setup steps.
>
> To test: install VB-CABLE from the link above (restart if asked), open Shuusshy, choose a
> microphone; the banner shows "Virtual mic live". In any recording app, choose "CABLE Output"
> as the microphone. Settings → Noise Suppression → Test plays the microphone before and after
> on headphones.
>
> Network: none, apart from the WebView2 runtime. Updates come from the Store (the built-in
> updater is off in this package). Audio is processed locally and never recorded or uploaded.
>
> Pro: an optional key, sold outside the app by email and checked offline; there are no
> purchases inside the app. The free features need no key.

## Things to know

- **Policy 10.2.4 (drivers):** Microsoft generally does not allow apps that depend on
  non-Microsoft drivers, but considers them case by case. Certification may ask questions or
  refuse; the notes above explain the dependency. If it is refused, the installer and portable
  ZIP on the website remain the way to get Shuusshy.
- **Pro keys:** non-game apps may sell outside the Store (policy 10.8.1). Shuusshy sells
  nothing inside the app.
- **Updates:** each new release needs a new package with a higher version (the release tag's
  version, with a fourth part of 0) and a new submission.
- **Testing locally** needs Windows *Developer Mode* (Settings → System → For developers), then
  `Add-AppxPackage -Register <layout>\AppxManifest.xml` on the unpacked layout.

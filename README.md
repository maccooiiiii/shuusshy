# Shuusshy

**Less Noise. More Voice.**

Shuusshy is a local, offline microphone noise-suppression utility for Windows 10/11. It cleans
your microphone on your PC (RNNoise, free; DeepFilterNet and voice isolation with Shuusshy Pro)
and passes it to Teams, Zoom, Discord and other apps through a virtual microphone (VB-CABLE,
included from 1.1.0 and installed only with your consent; VB-CABLE is a donationware by
VB-Audio Software, www.vb-cable.com, all participations are welcome).

> **Status: version 1.0.1 (stable), released 2026-11-16, valid until 2028-11-16.**
> Website: <https://maccooiiiii.github.io/shuusshy/> · Download: [Releases](https://github.com/maccooiiiii/shuusshy/releases/latest)
> - [Release notes](docs/RELEASE_NOTES_1.0.0.md)
> - [Known limitations](docs/KNOWN_LIMITATIONS.md)
> - [Acceptance checklist](docs/RELEASE_1.0.0_CHECKLIST.md)
> - [Installation](INSTALLATION.md)
> - [Project status](PROJECT_STATUS.md)

## Principles

- All audio processing happens on your computer. No account, no cloud, no uploads.
- No recording of microphone audio.
- Every indicator in the UI reflects real state. Nothing is simulated.

## Developing

Prerequisites (Windows 10/11 x64):

- Rust stable (`x86_64-pc-windows-msvc`) via [rustup](https://rustup.rs)
- Visual Studio 2022+ or Build Tools with "Desktop development with C++"
- Node.js 20+ and npm
- WebView2 Runtime (preinstalled on Windows 11)

> Keep the checkout in a path without `#` characters. Vite cannot resolve modules under such paths.

```bash
npm install
npm run tauri dev      # run the app
npm run typecheck      # TypeScript
npm test               # frontend tests (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture   # hardware tests (real audio devices)
npm run tauri build    # release build + NSIS installer (src-tauri/target/release/bundle/nsis)
npm run package:portable   # portable ZIP from the release build
```

Installing and the portable edition for users: [INSTALLATION.md](INSTALLATION.md).

## Layout

```
src/              React + TypeScript banner UI
src/lib/          IPC wrappers and UI state models
src-tauri/src/    Rust backend (commands, settings store; audio engine in later milestones)
docs/             Architecture, licensing and milestone reports
tools/blender/    Blender script that renders the animated emblem and tray frames
tools/license-keygen/  Offline tool to issue Shuusshy Pro keys (signing key stays outside the repo)
.github/          CI workflows
```

## Credits

Shuusshy is created by **Ian Musa** (design and development).

It builds on RNNoise (via nnnoiseless), DeepFilterNet, VB-CABLE by VB-Audio, Tauri, React and
Rust. See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## License

**Source-available, all rights reserved** © 2026 Ian Musa. See [LICENSE](LICENSE). You may
read, build and evaluate the code; copying, modifying or redistributing it needs permission.
Third-party components keep their own licenses ([THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md)).

Official releases are valid for two years from their release date and Pro features need a
license key; see [docs/RELEASES.md](docs/RELEASES.md) and [docs/LICENSING.md](docs/LICENSING.md).

More: [INSTALLATION.md](INSTALLATION.md) · [PRIVACY.md](PRIVACY.md) · [SECURITY.md](SECURITY.md) ·
[CONTRIBUTING.md](CONTRIBUTING.md)

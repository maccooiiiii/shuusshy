# Shuusshy

**Less Noise. More Voice.**

Shuusshy is a free, open-source, offline microphone noise-suppression utility for Windows. It
cleans your microphone audio locally (RNNoise or DeepFilterNet) and passes it to apps like Teams,
Zoom, Discord and browsers through a virtual microphone (VB-CABLE).

> **Status: early development, not usable yet.** Milestones 1–2 (desktop shell, microphone and
> VB-CABLE detection) are done. The app does not capture or process audio yet. See [PROJECT_STATUS.md](PROJECT_STATUS.md).

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

Installing and the portable edition for users: [docs/INSTALL.md](docs/INSTALL.md).

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

Not chosen yet. The choice is pending a decision on the release-expiration policy. See
docs/ARCHITECTURE.md § Open decisions.

# Third-Party Licenses

Shuusshy builds on the open-source components below. This list covers direct dependencies that
ship in the application. Transitive dependencies are mostly MIT and/or Apache-2.0 licensed. A
complete generated inventory (for example with `cargo about`) will be added before the 1.0.0
release (Milestone 12).

_Last reviewed: Milestone 5._

## Audio

| Component | Version | License | Use |
|---|---|---|---|
| [nnnoiseless](https://github.com/jneem/nnnoiseless) (Rust port of Xiph **RNNoise**, including its trained model weights) | 0.5.2 | **BSD-3-Clause** | RNNoise noise suppression engine |
| [dfn3_rs](https://github.com/TanixLu/dfn3_rs) (DeepFilterNet3 inference, embedded model) | 0.1.2 | MIT OR Apache-2.0 | DeepFilterNet noise suppression engine |
| [cpal](https://github.com/RustAudio/cpal) | 0.18.2 | Apache-2.0 | Audio device streams (WASAPI) |
| [rubato](https://github.com/HEnquist/rubato) | 5.1.0 | MIT OR Apache-2.0 | Sample-rate conversion, clock-drift correction |
| [rtrb](https://github.com/mgeier/rtrb) | 0.4.1 | MIT OR Apache-2.0 | Lock-free ring buffers |
| [windows](https://github.com/microsoft/windows-rs) | 0.62.2 | MIT OR Apache-2.0 | WASAPI / MMDevice / session APIs |

**RNNoise / nnnoiseless (BSD-3-Clause).** Copyright (c) 2020 Joe Neeman; (c) 2017 Mozilla;
(c) 2007-2017 Jean-Marc Valin; (c) 2005-2017 Xiph.Org Foundation; (c) 2003-2004 Mark Borgerding.
Redistribution in binary form must reproduce the copyright notice, the list of conditions and
the disclaimer. The full license text will be included with every binary release (installer and
portable ZIP) and in the application's About section (Milestone 8).

**DeepFilterNet / dfn3_rs (MIT OR Apache-2.0).** DSP, streaming conventions and model
parameters derive from [DeepFilterNet](https://github.com/Rikorose/DeepFilterNet) by Hendrik
Schröter and contributors. Upstream states that "all code and model weights" are dual-licensed
under MIT or Apache-2.0 (README, since commit `7b14b89`; verified on GitHub during Milestone 5).
There are no non-commercial or research-only conditions. `dfn3_rs` is an independent
implementation (Tanix Lu), not an official DeepFilterNet release. Its weights are the upstream
DeepFilterNet3 model, repacked (fp16 decoder). As upstream requests, Shuusshy credits the paper:
H. Schröter et al., "DeepFilterNet: Perceptually Motivated Real-Time Speech Enhancement",
INTERSPEECH 2023.

## Licensing

- **ed25519-dalek** 2 (BSD-3-Clause), with curve25519-dalek (BSD-3-Clause): offline license
  key verification. The key tool also uses rand_core (MIT/Apache-2.0); it is not shipped.

## Application framework and storage

| Component | Version | License |
|---|---|---|
| [Tauri](https://tauri.app) | 2.12.5 | Apache-2.0 OR MIT |
| [tauri-plugin-single-instance](https://github.com/tauri-apps/plugins-workspace) | 2 | Apache-2.0 OR MIT |
| [rusqlite](https://github.com/rusqlite/rusqlite) / libsqlite3-sys | 0.42.0 / 0.40.0 | MIT |
| [SQLite](https://sqlite.org) (bundled) | — | Public domain |
| [serde](https://serde.rs), serde_json | 1.x | MIT OR Apache-2.0 |
| [thiserror](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 |

## User interface

| Component | Version | License |
|---|---|---|
| [React](https://react.dev), react-dom | 19.3.1 | MIT |
| [@tauri-apps/api](https://tauri.app) | 2.12.4 | Apache-2.0 OR MIT |
| [Tailwind CSS](https://tailwindcss.com) | 4.3.5 | MIT (build-time) |
| [Vite](https://vite.dev) | 8.3.8 | MIT (build-time) |

## Installer

- **NSIS** (Nullsoft Scriptable Install System, zlib/libpng licence) builds the Windows
  installer via Tauri's bundler. The installer's artwork is Shuusshy's own (rendered with
  Blender from `tools/blender/render_emblem.py`).

## Distributed with Shuusshy (Windows), unmodified

- **VB-CABLE** by VB-Audio Software, <https://vb-audio.com/Cable/> (www.vb-cable.com).
  VB-CABLE is a donationware, all participations are welcome.
  - From version 1.1.0 the Windows installer and portable ZIP include the official VB-CABLE
    package **as is** (unmodified, `vbcable` folder), under VB-Audio's distribution terms:
    <https://vb-audio.com/Services/licensing.htm>. Its own licence is in `vbcable/readme.txt`.
  - It is installed only if you agree: the Shuusshy installer asks, then VB-Audio's own setup
    opens with Windows' administrator prompt. It can also be started from Settings → Audio
    Output.
  - The package is fetched from VB-Audio's server by `scripts/fetch-vbcable.ps1`, which checks
    its SHA-256 and VB-Audio's code signature. It is not stored in this repository.
  - Organisations deploying Shuusshy to staff who cannot identify or support VB-CABLE
    themselves must buy VB-CABLE licences from VB-Audio (see the licensing page).

## Not bundled

- **BlackHole** (Existential Audio, GPL-3.0) is the virtual cable for the macOS version. Users
  install it themselves from <https://existential.audio/blackhole/>. Bundling it with a
  non-GPL app requires a licence from Existential Audio.

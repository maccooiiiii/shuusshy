# Shuusshy Audio Engine

_Last updated: Milestone 7 (live test, meters, monitoring)._

## Signal path

```
Physical mic ──WASAPI shared──► capture callback ──ring_in──► DSP thread ──ring_out──► output callback ──► "CABLE Input"
 (native rate/ch)               downmix to mono    (500 ms)   │                (500 ms)   upmix to all ch     │
                                                              ├ resample → 48 kHz (only if mic ≠ 48 kHz)        ▼
                                                              ├ cut into 480-sample (10 ms) frames         VB-CABLE driver
                                                              ├ FrameProcessor (Passthrough in M3;              │
                                                              │   RNNoise / DeepFilterNet from M4/M5)            ▼
                                                              └ drift-corrected resample → cable rate   "CABLE Output" → Teams/Zoom/…
```

| Stage | Thread | Rules |
|---|---|---|
| Capture callback | cpal / WASAPI audio thread | No locks, allocations or I/O. Downmixes into a lock-free SPSC ring (`rtrb`). If the ring is full, drops audio and counts an *overrun* |
| DSP | `shuusshy-dsp`, registered with MMCSS "Pro Audio" | All buffers preallocated. Polls the ring every 1 ms when idle |
| Output callback | cpal / WASAPI audio thread | No locks or allocations. Starts playing only once a pull would leave the 20 ms target margin. Pads with silence on *underrun* and re-primes. If a pull would leave more than 60 ms, trims back to the target (*resync*) |

Internal format: **48 kHz mono f32, 480-sample frames.** This is the native rate and hop size of
both RNNoise and DeepFilterNet, so the engines (Milestones 4–5) need no further conversion.

## RNNoise engine (Milestone 4)

`audio/engines/rnnoise.rs`, using `nnnoiseless` 0.5.2 (pure-Rust RNNoise, BSD-3-Clause).

| Aspect | Implementation |
|---|---|
| Format | 48 kHz mono, 480-sample frames, samples scaled to the 16-bit range (×32768) as RNNoise requires |
| Delay | **One frame (10 ms)**, measured by unit test (`delay_is_one_frame`). The delay is kept when suppression is off, so switching never shifts the audio in time |
| On/off | **Genuine bypass:** off = the original audio, untouched, delayed by the same frame (bit-exact, unit-tested) |
| Strength | RNNoise has no strength parameter. Shuusshy uses a documented **wet/dry mix**: `out = s·denoised + (1−s)·original` (sample-aligned). Exact mix verified by unit test. A mix of `s` limits attenuation to roughly `20·log10(1−s)` dB |
| Switching | Every on/off/strength change is ramped linearly over one frame (verified sample-exact), so it cannot click |
| Live control | `EngineShared` atomics. The controller changes on/off/strength without restarting the pipeline |
| Real-time safety | Engine allocated off the audio thread; RNNoise processing uses preallocated buffers. Unusable output (NaN/inf) is replaced by the original audio and counted |
| Warm state | RNNoise keeps running while bypassed (~1 % of a core), so switching on is instant without fade-in artifacts |
| Status | `processing` / `bypassed` only while audio actually flows; otherwise `unavailable` (with the routing reason), `initializing`, `interrupted` or `error` |

**Measured on the development PC** (`examples/denoise_check.rs`: real Realtek mic → pipeline →
CABLE Output, speech with pauses plus background noise; aligned by capture timestamps; speech
and noise separated by a voice detector on the raw mic):

| Measurement | Result |
|---|---|
| Noise removed where no voice is detected (247 × 20 ms blocks) | **24.3 dB** median (noise floor −53.7 → −80.1 dBFS) |
| Speech level change where voice is detected (152 blocks) | **−0.7 dB** median, −1.7 dB at the 25th percentile |
| Bypass (off) | **0.00 dB** level change |
| Engine CPU | **0.13–0.14 ms per 10 ms frame** (≈1.4 % of one core, i7-7700HQ); 0 unusable frames |
| Pipeline during the test | 0 underruns, 0 overruns, 0 resyncs |

Lessons from building the measurement:
- **Synthetic white noise is a poor RNNoise test.** Its reduction varied from 1 to 30 dB with
  the random seed alone. Unit tests therefore verify Shuusshy's integration exactly (bypass, mix,
  ramp, delay, scaling) rather than RNNoise's quality, and quality is measured on real audio.
- **Align mic and cable recordings by capture timestamps, not by correlation.** The two
  devices' clocks differ by ~100 ppm, which smears a 10 s single-lag correlation (r ≈ 0.4 even in
  bypass). Block levels are unaffected.

## DeepFilterNet engine (Milestone 5)

`audio/engines/deepfilter.rs`, using `dfn3_rs` 0.1.2 (DeepFilterNet3 inference with the model
embedded; MIT OR Apache-2.0 for code **and** weights, as stated in the upstream README since
commit `7b14b89`, verified on GitHub).

**Choosing the implementation (spike on the development PC, 20 s test signal, release build):**

| | `deep-filter-rt` 0.2 (official tract inference) | **`dfn3_rs` 0.1 (chosen)** |
|---|---|---|
| Average / worst time per 10 ms frame | 0.75 ms / **202 ms** | **0.20 ms / 4.1 ms** |
| Heap allocations while processing (counting allocator) | **206 per frame** | **0** |
| Start-up | 173 ms | 0 ms (weights referenced in place) |
| Output vs official implementation | (reference) | difference 57.5 dB below the signal |

The official path allocates every frame and had a 202 ms spike, which is unacceptable on a
real-time audio thread. `dfn3_rs` reorders the weights (fp16 decoder), but its output is
numerically equivalent for practical purposes.

| Aspect | Implementation |
|---|---|
| Format / delay | 48 kHz mono, 480-sample frames, normalized f32. **Delay 1440 samples (30 ms)**, measured by test, kept when off |
| Strength | DeepFilterNet's own **attenuation limit**: 0 % = 0 dB (exact original), linear to 45 dB at 99 %, 100 % = unlimited. Changes ramp over 50 ms |
| Off | Limit 0 dB = exact delayed input (bit-exact test). A stable 0 dB also skips the network: 0.05 ms/frame when bypassed |
| Overload watchdog | More than 7 ms per frame for 20 consecutive frames → pass audio through untouched, status *Error: too slow, choose RNNoise* |
| Unusable output | NaN/inf frame → silence for that frame, model state reset, counted |
| Engine switch | Restarts the pipeline with the new engine (different delays); on/off and strength stay live |

**Development builds.** Unoptimized, DeepFilterNet runs far slower than real time (the watchdog
trips) and RNNoise can drop audio under load. `Cargo.toml` therefore compiles the audio libraries
(dfn3_rs, nnnoiseless, rubato and the FFT crates) at `opt-level = 3` even in the dev profile.
RNNoise went from 2.5 to 0.17 ms per frame in `tauri dev`, and DeepFilterNet runs at 0.47 ms.

### Engine comparison with a known clean reference

`examples/engine_compare.rs` uses real sentences (Windows text-to-speech, male "David" and
female "Zira", 22 s each), mixed with fan-like noise and keyboard clicks (−32.5 dBFS). They run
offline through the real processors and are compared with the clean speech. SI-SDR is the
scale-invariant signal-to-distortion ratio (higher = closer to clean speech).

| (David / Zira) | Speech level | Noise in pauses | SI-SDR |
|---|---|---|---|
| Noisy input | — | −32.5 dBFS | 9.3 / 9.8 dB |
| RNNoise 80 % | −0.3 / −0.5 dB | −45.6 dBFS | 10.5 / 10.0 dB |
| **DeepFilterNet 80 %** | −0.7 / −0.5 dB | −53.9 / −53.3 dBFS | **12.2 / 15.7 dB** |
| RNNoise 100 % | −0.4 / −0.5 dB | −57.7 / −59.4 dBFS | 9.4 / 8.6 dB |
| DeepFilterNet 100 % | −0.7 / −0.5 dB | −54.3 / −53.6 dBFS | 12.1 / 15.6 dB |

- Both engines keep speech level within 1 dB.
- DeepFilterNet improves SI-SDR by 3–6 dB over the noisy input; RNNoise by 0–1 dB. At 100 %,
  RNNoise removes more noise but distorts the voice more (SI-SDR below the noisy input).

**Live check on the user's microphone** (`denoise_check`, both engines, same session):
- both bypasses 0.00 dB;
- noise removed: 32 dB with RNNoise, to digital silence with DeepFilterNet;
- 0 dropouts; 0.13 / 0.57 ms per frame.

Its speech figures (−1.3 / −10.4 dB) rest on only 27–36 speech blocks (~0.5 s), so they are
**not** considered reliable; the controlled comparison above supersedes them. A synthetic
harmonic "voice" was also attenuated by ~7 dB by DeepFilterNet. That shows synthetic buzz is not
a valid speech test, not that speech is lost.

## Processing chain: gains, noise gate, limiter (Milestone 6)

`audio/chain.rs` wraps the selected engine; every frame goes through:

```text
input gain → engine → noise gate → output gain → safety limiter → clamp
```

| Stage | Behaviour |
|---|---|
| Input / output gain | −24…+24 dB; a change ramps linearly over one frame (no zipper noise). Input gain sits before the engine so a quiet mic is raised before suppression |
| Noise gate (optional) | Per 10 ms frame on the engine output: opens above the threshold; closes only after 200 ms (hold) at least 4 dB below it (hysteresis). Gain glides: ~2 ms to open, ~50 ms to close, floor −80 dB. Disabled = exactly transparent |
| Safety limiter | Peak limiter at −0.5 dBFS, instant attack, 100 ms release; exactly transparent below the threshold. Frames limited by more than 1 dB are counted and shown in Settings |

Noise cancellation on/off still bypasses only the engine; gains and the gate are separate,
explicitly enabled controls. All are live (atomics), with no audio restart.

**Unit tests**: neutral chain is bit-exact; gains measure within 0.05 dB; gain changes have no
steps; the gate mutes −66 dBFS noise by more than 60 dB and leaves −23 dBFS untouched; the gate
does not close during 100 ms pauses or chatter inside the hysteresis band; open/close are smooth;
the limiter keeps +12 dB output gain at or below −0.5 dBFS and counts it.

**Hardware check** (`examples/levels_check.rs`, real pipeline, room noise as signal, each setting
vs a neutral control): input +6 → **+6.0 dB**; output −10 → **−9.8 dB**; +12/−6 → **+6.0 dB**;
gate above the room level → **−107.5 dB**; gate at −90 dB → **0.0 dB**; 0 dropouts. The neutral
control itself read −0.6 dB because the microphone was delivering only its digital noise floor
(−92 dBFS), whose top band the resampler filters out. That is why every setting is compared with
the control.

## Presets (Milestone 6)

`settings/presets.rs`. Each preset is a concrete engine configuration. Applying one sets
engine, strength and gate, and switches noise cancellation on; gains are left alone. The UI
shows the preset whose values the settings currently match, otherwise **Custom**.

| Preset | Engine | Strength | Noise gate |
|---|---|---|---|
| Balanced | DeepFilterNet | 80 % (limit ≈ 36 dB) | off |
| Strong Noise Reduction | DeepFilterNet | 100 % (unlimited) | off; **voice isolation 6 dB** |
| Speech Priority | DeepFilterNet | 50 % (limit ≈ 22 dB) | off |
| Low CPU Usage | RNNoise | 80 % | off |

The choices follow the Milestone 5 measurements: DeepFilterNet gave the cleanest speech, a
lower attenuation limit keeps the voice most natural, and RNNoise costs about a quarter of
DeepFilterNet's CPU with a 10 ms instead of 30 ms delay.

## Voice isolation (noise and background-voice improvements)

**Goal:** suppress other people talking in the background, which both engines are trained to
*keep*, without hurting the user's own voice.

**Benchmark** (`examples/isolation_bench.rs`, offline, known clean reference):
- target voice: Windows TTS "David", 8 sentences with 1.8 s pauses, −20 dBFS;
- background talker: "Zira", continuous, 12 / 9 / 6 dB quieter;
- fan noise and keyboard clicks (−32.4 dBFS);
- also a speaker whose loudness swings ±5 dB.

Metrics:
- SI-SDR vs the clean voice;
- voice level change;
- **background leak** (output vs background input where only the background talks);
- noise floor where nobody talks;
- **voice blocks cut**: share of 20 ms voice blocks attenuated by more than 6 dB vs
  DeepFilterNet alone.

**Candidates tried** (background −12 dB, steady voice):

| Configuration | SI-SDR | Voice | Background leak | Noise floor | Verdict |
|---|---|---|---|---|---|
| DeepFilterNet 100 % (before) | 9.7 dB | −0.6 dB | −2.3 dB | −59.5 dBFS | baseline: keeps background speech |
| RNNoise 100 % | 7.4 dB | −0.3 dB | −1.7 dB | −60.2 dBFS | keeps background speech |
| DeepFilterNet → RNNoise cascade | 7.7 dB | −0.8 dB | −2.5 dB | −79.0 dBFS | more distortion, rejected |
| hush-vani (16 kHz "background-speaker" model) | 2.1 dB | **−14.2 dB** | −17.3 dB | −53.1 dBFS | attenuates *all* speech, 16 kHz only, rejected |
| DeepFilterNet + fixed gate −26 dB | 11.4 dB | −0.6 dB | −14.0 dB | −68.3 dBFS | works only if the threshold happens to fit the user's level |
| **DeepFilterNet + voice isolation, 6 dB (shipped)** | **11.6 dB** | **−0.6 dB** | **−21.8 dB** | **−68.3 dBFS** | adaptive, chosen |

**How it works** (`audio/chain.rs`):
- **Voice tracker.** The user is much louder at their own microphone than a background talker.
  A tracker follows the upper envelope of the cleaned output's level. It rises by 10 % of the gap
  per frame (one bump barely moves it) and decays 1 dB/s.
- **Adaptive gate.** The gate threshold is the tracked voice level minus the margin. During
  pauses, quieter speech is muted (down to −80 dB) after the 200 ms hold.
- **Look-ahead, no added delay.** The engine input runs 3 frames (DeepFilterNet) or 1 frame
  (RNNoise) ahead of its output. A separate RNNoise instance is used only as a speech detector on
  that input. The gate opens early when upcoming audio is speech **and** at least 6 dB above the
  threshold, which roughly halved clipped word onsets.

Two look-ahead designs failed the benchmark first:
- **Level-only look-ahead:** keyboard clicks opened the gate, and background leak fell from
  −22 to −4 dB.
- **Speech-only look-ahead:** background speech plus noise in the raw input opened the gate
  (−6 dB leak).

**Margins** (with the shipped look-ahead):

| Scenario | 3 dB | 4.5 dB | **6 dB (default)** | 9 dB |
|---|---|---|---|---|
| Background −12 dB, steady voice: leak / voice cut | −23.9 / 2.2 % | −23.6 / 1.5 % | **−21.8 / 0.9 %** | −5.8 / 0.6 % |
| Background −6 dB, steady voice | −6.0 / 1.6 % | −2.7 / 1.1 % | −2.0 / 0.7 % | −1.5 / 0.2 % |
| Background −12 dB, voice ±5 dB | −27.3 / 5.7 % | −26.8 / 2.6 % | **−22.7 / 1.3 %** | −8.1 / 0.9 % |
| Background −9 dB, voice ±5 dB | −23.3 / 5.5 % | −10.9 / 2.6 % | −8.2 / 1.3 % | −2.7 / 0.8 % |

In every scenario and margin, isolation improved SI-SDR over DeepFilterNet alone and kept the
voice level within 0.1 dB. It also lowers the residual noise in pauses by ~9 dB (−59.5 → −68.3
dBFS).

**Limits** (stated in the UI too):
- It cannot separate voices talking **at the same time**.
- A background talker less than ~6 dB quieter than the user is only partly suppressed.
- About 1 % of voice blocks are still attenuated at the default margin, mostly the first
  milliseconds of a sentence after silence.
- The detector adds ~0.13 ms per frame, and only while a gate is enabled.

**Real app:** applying "Strong Noise Reduction" switched on isolation (6 dB). The live readout
showed the tracked voice (−25 dBFS), the muting threshold (−31 dBFS) and the gate closing during
a pause.

## Live test, meters and monitoring (Milestone 7)

**Meters** (`audio/chain.rs`, `Meter`). Every 10 ms frame, the chain measures:
- **before suppression**: after input gain, before the engine;
- **final output**: what apps receive.

Ballistics: RMS integrated over ~300 ms, peak hold falling 15 dB/s, floor −100 dBFS. Values are
published through atomics and polled by the UI (~20 Hz in the test panel, ~10 Hz for the
banner's activity bar). Nothing is estimated or animated in the UI: with no audio the meters are
empty, and the readout shows "—" or "no audio".

**Monitoring** (headphones):
- **Taps.** The chain copies either the *original* or the *processed* signal into a monitor
  ring. The original is delayed by the engine's own delay, so switching never jumps in time.
  A unit test proves it: through a delay-only engine, original and processed monitor streams are
  sample-identical. Only one signal is ever copied, and `off` copies nothing.
- **Playback.** `Pipeline::start_playback` is the main pipeline's machinery (ring, resampling,
  clock-drift correction) with the monitor ring as input and the chosen headphones as output.
  It is independent of the virtual microphone, which keeps running.
- **Real-time safety.** Monitor rings are handed to the audio thread through a lock-free mailbox,
  and replaced rings are handed back to be freed elsewhere. The audio thread never allocates or
  frees. A full monitor ring only drops monitor audio.
- **Safety.**
  - Starts only on an explicit click; Stop, Esc or closing the panel stops it, muting the tap
    first.
  - VB-CABLE cannot be chosen as a monitor device (feedback loop).
  - Monitoring stops if the main audio restarts or the monitor device fails.
  - Headphones are recommended; devices whose name suggests loudspeakers get a feedback warning.
  - Nothing is recorded or saved.

**Verification**:
- **Unit tests:** meter levels exact (−23.0 dBFS RMS / −20.0 dBFS peak sine, output follows
  gain), peak falls 15 dB/s, silence reaches the floor; original/processed aligned; off writes
  nothing; replaced rings returned, not freed on the audio thread.
- **Real app:** meters showed the room at −54 dB before suppression and "—" after it. While
  talking, −31 / −41 dB.
- **Monitoring:** started on the user's Razer headset ("Speakers (7.1 Surround Sound)", 8 ch),
  switched original → processed, and stopped with Esc. The user heard original, then processed,
  then silence. The virtual mic stayed live throughout.
- **Loopback check:** `examples/monitor_check.rs` (WASAPI loopback of the monitor device) read
  exact digital silence on this Razer *virtual* surround device even while the user heard the
  audio. Loopback cannot observe this device, so that automated check is inconclusive here; the
  user's listening is the evidence.

## Clock drift compensation

The microphone and VB-CABLE run on independent clocks. Measured on the development PC, they
differ by tens of ppm (one 5-minute run settled at about +25 ppm). Left uncorrected, 50 ppm grows
or drains the buffer by 3 ms per minute, which means dropouts or ever-growing latency within
minutes.

**VB-CABLE pulls audio in bursts.** Each output callback asks for 960 frames (20 ms),
occasionally 1056, normally every 20 ms but sometimes 30–47 ms late. The buffer level is
therefore a sawtooth, and "how full is the buffer" depends on when you look.

**Final design** (`drift.rs`, `io.rs`, `dsp.rs`):
- The output callback measures the **margin**: what is left in the buffer right after each pull.
- One definition drives everything: playback (re)starts only once a pull would leave the
  **20 ms target margin**, and a pull that would leave more than **60 ms** trims back to target.
- Once per second the DSP thread feeds the **average margin** of that second to a PI controller
  that nudges the output resampler (rubato `Async` sinc, ramped per chunk, max ±2000 ppm).
- PI tuned from the process itself (1 ppm moves the buffer 0.048 samples/s): critically damped,
  ~30 s settling time.
- Transients are kept out of the drift estimate. The integral only accumulates errors within
  ±10 ms, and the proportional response is capped. A stall's backlog is drained by the
  controller over ~20 s (≤ ~2 cents of pitch, inaudible for speech) or resynced if very large.

**Designs that failed on hardware** (kept here so they are not retried):
1. *Steer the instantaneous level to 20 ms.* It chased the burst sawtooth, swinging the
   correction between ±1700 ppm.
2. *Steer the post-pull low-water mark to 10 ms.* The margin was too thin for ordinary jitter:
   16 underruns in 5 minutes, 6 of them with the PC idle.
3. *Steer a 1-second average of the level, but re-prime to a different reference point.* Each
   underrun inflated the average, the controller over-corrected, and a correction/underrun
   spiral followed (found in simulation before shipping).

**Unit tests** model the measured timing: 10 ms mic packets with occasional one-packet lateness,
and 20 ms pulls with occasional late pulls. They check that the controller:
- learns −200…+150 ppm to within 15 ppm with zero underruns or resyncs,
- holds the average margin within 1 ms of target,
- stays calm (worst ±120 ppm, mean ±15 ppm) with no drift,
- recovers from repeated 50 ms system stalls, costing at most one dropout each, without
  corrupting the learned drift.

## Measured results (development PC, Windows 10, Realtek mic → VB-CABLE, 48 kHz)

| Measurement | Result |
|---|---|
| Routing proof (mic vs CABLE Output cross-correlation) | 0.983 / 0.997 / 0.967 in three runs; control at +200 ms ≈ 0. Levels identical (e.g. −23.2 dBFS both) |
| End-to-end latency, mic capture → CABLE Output capture | 124–127 ms |
| of which VB-CABLE itself (`cable_latency`) | **80.6 ms** (correlation 1.0000) |
| of which Shuusshy (capture period + buffer + output queue) | about 45 ms (average buffer 25–30 ms) |
| Final design, 5-minute soak: idle 0–120 s | **0 underruns**, drift settled at about +25 ppm, steady within ±10 ppm, lowest margin ~18 ms |
| Final design, 5-minute soak: total incl. 150 s with 8 busy CPU threads | 2 underruns, both during system-wide audio stalls (devices silent for 40 ms and 89 ms, WASAPI glitch flags raised at the same moment). No resyncs, no errors |
| CPU per 10 ms frame (resampling only) | typically < 0.3 ms, worst 3.4 ms under load |

Notes:
- The final build's soak run had a silent microphone (−91 dBFS), so it measured stability, not
  correlation. The routing proof figures come from the earlier runs. The signal path itself was
  unchanged by the buffering fixes.
- **The ~100 ms latency target is not met with VB-CABLE's default settings**, because VB-CABLE
  alone adds about 81 ms. Shuusshy's own share is about 45 ms. Possible future work: VB-CABLE's
  internal buffer settings, or a native virtual microphone (spec Part 5.4).
- A system-wide audio stall longer than the 20 ms margin causes one dropout in Shuusshy as in
  every other audio app. Shuusshy recovers automatically.

## Controller and recovery (`controller.rs`)

A single `shuusshy-audio` thread owns the pipeline and *reconciles* what should run against what
is running:

- **Should run** when: routing is enabled, the selected mic is available, and VB-CABLE is ready.
  Otherwise the state is `off` or `waiting` (with the reason).
- Reconciles on device hot-plug, microphone selection, settings changes, and when a scheduled
  retry comes due.
- **Stream errors** (device removed, invalidated) → `interrupted`, pipeline dropped, retried with
  backoff (2 s doubling to 30 s). Errors from an already replaced pipeline are ignored using a
  generation counter.
- **Stalls**: if either stream's callbacks stop for 2 s, the pipeline is restarted.
- **Glitch notices** (`Xrun`, WASAPI data discontinuity) are counted, not treated as failures. One
  is normal when a stream starts.
- `running` is reported only after both streams started successfully.
- On app exit the pipeline is stopped so VB-CABLE goes silent cleanly.

## Virtual-cable conflicts

VB-CABLE's playback endpoints **CABLE Input** and **CABLE In 16ch** feed one cable. Verified on
hardware:
- while one is in use, opening the other fails with `AUDCLNT_E_DEVICE_IN_USE` (cpal
  `DeviceBusy`), whichever app started first wins;
- two apps on the *same* endpoint (shared mode) are mixed into CABLE Output.

A real case: Discord with its *speaker* set to the cable locked Shuusshy out and looped its own
playback into its microphone. The device monitor polls audio sessions on all VB-CABLE playback
endpoints every 2 s, excluding Shuusshy's own process and Windows audio hosts, and publishes the
other apps' names as `cableUsers`. The UI then warns by name. Exclusive-mode holders have no
session, so they show only as the "cable busy" error.

## Verification tools

Both live in `src-tauri/examples/` and drive the real code on real devices. Close the Shuusshy
app first: two writers to CABLE Input would mix.

```bash
cd src-tauri
cargo run --release --example route_check -- 6        # routing proof + latency
cargo run --release --example route_check -- 6 300 8 150   # ...plus a 5-min soak, 8 busy threads from 150 s
cargo run --release --example cable_latency            # VB-CABLE's own latency
cargo run --release --example mic_level -- 4           # mute/volume + real signal level per input (read-only)
cargo run --release --example who_uses                 # which processes have audio sessions where (read-only)
cargo run --release --example denoise_check -- 8       # both engines on the real mic: noise removed, speech, bypass
cargo run --release --example engine_compare -- a.wav  # both engines vs clean speech WAVs (48 kHz 16-bit mono)
cargo run --release --example levels_check            # gains and noise gate through the real pipeline (no talking)
cargo run --release --example isolation_bench -- t.wav bg.wav  # noise + background-voice benchmark
cargo run --release --example monitor_check -- "headset" 6   # what the monitor device plays (loopback; read-only)
cargo run --release --example cable_occupy -- input 15 # plays silence into the cable to test the conflict warning
```

Run `mic_level` first if `route_check` reports a near-silent microphone (around −90 dBFS).
Correlation needs real signal, so a silent mic makes the proof inconclusive, not failed.

`route_check` starts the real pipeline. It then records the microphone directly **and**
`CABLE Output` at the same time and cross-correlates them, using the two streams' capture
timestamps:

- A high correlation proves that what apps receive is the microphone's audio.
- The lag gives the true end-to-end latency.
- A control correlation at +200 ms must be low.

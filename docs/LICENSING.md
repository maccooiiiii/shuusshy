# Shuusshy Pro licensing

Shuusshy has a free tier and Pro features unlocked by an offline license key.

| Free | Pro (key) |
|---|---|
| RNNoise noise cancellation | DeepFilterNet engine |
| Virtual microphone, live test, meters | Voice isolation |
| Input/output gain, noise gate, limiter | Balanced, Strong Noise Reduction and Speech Priority presets |
| Low CPU Usage preset | |

## How keys work

- A key looks like `SHUU-XXXXX-XXXXX-…` (119 characters plus dashes). It carries a serial
  number, the issue date and an optional expiry date. These are signed with Ed25519.
- Shuusshy checks the signature **on the PC** against the public key built into the app
  (`src-tauri/src/license.rs`). It needs no account and no internet, so it matches the privacy
  rules.
- The key is stored in the local settings database. Removing it switches Pro features off.
  Free features keep working, and the user's DeepFilterNet/isolation choices are kept for
  later.
- Gating is enforced in the backend (`commands::processing_from`, `apply_preset`). The UI only
  mirrors it with PRO badges.
- Users enter the key after installation: the first-run card says "Have a key?", and the
  field is under Settings → Application and Updates.
- **Limitation:** Shuusshy is open source, so someone can rebuild it without the check. Keys
  stop casual sharing, not determined copying. Keys cannot be revoked (no server); use an
  expiry date if you need time-limited keys.

## Issuing keys (`tools/license-keygen`)

```bash
cd tools/license-keygen
cargo run --release -- init                         # once: creates the signing key
cargo run --release -- issue 2                      # perpetual Pro key, serial 2
cargo run --release -- issue 3 2027-12-31           # key valid until 31 Dec 2027
cargo run --release -- pubkey                       # public key to paste into license.rs
```

- The **private signing key** is at `%USERPROFILE%\.shuusshy\license-signing.key`, outside
  the repository (override with `SHUUSSHY_SIGNING_KEY`). Back it up offline. Anyone with it can
  issue keys. If it is lost, no new keys can be issued for the current public key.
- Keep your own list of issued serials, for example in `%USERPROFILE%\.shuusshy\issued-keys.txt`.
- Changing the key pair (new `init` + new public key in `license.rs`) invalidates every key
  issued before.
- The key format lives in `src-tauri/src/license_format.rs`, which both the app and the tool
  compile, so they cannot drift apart.

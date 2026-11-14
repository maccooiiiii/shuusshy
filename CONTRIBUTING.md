# Contributing

Thanks for helping improve Shuusshy. The project is **source-available**, not open source (see
[LICENSE](LICENSE)). By opening a pull request, you agree that your contribution may be used and
distributed as part of Shuusshy under that license.

## Before you start

- For anything larger than a small fix, open an issue first to agree on the approach.
- Keep the product principles in mind: local processing only, real indicators (never simulated
  meters or status), a compact banner instead of dashboards, and no hidden behaviour.

## Development setup

See [README.md](README.md#developing). In short:

```bash
npm install
npm run tauri dev
```

Keep the checkout in a path without `#` (Vite limitation).

## Checks (CI runs the same)

```bash
npm run typecheck
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Audio changes should include a measurement, not only "it sounds better". The tools in
`src-tauri/examples/` (route_check, isolation_bench, …) produce numbers to compare against.

## Pull requests

- One topic per pull request, with a clear description of what changed and how it was tested.
- Update `CHANGELOG.md` (Unreleased) and the relevant docs.
- Never commit secrets: license or updater signing keys, `.env` files, certificates.

# Security

## Reporting a vulnerability

Please report security problems privately, not in a public issue. Use GitHub's **"Report a
vulnerability"** (Security → Advisories) on the official repository, or contact the maintainer
through the repository. Include the version (Settings → About), steps to reproduce and impact.
You will get an answer as soon as possible.

## Supported versions

Each official release is supported for its validity period of two years from its release date
(see [docs/RELEASES.md](docs/RELEASES.md)). Fixes ship in new releases.

## How Shuusshy protects you

- **Local processing.** No account; Shuusshy's only network use is the optional update check.
  The WebView2 runtime may contact Microsoft on its own ([PRIVACY.md](PRIVACY.md)).
- **Restricted app permissions.** Tauri capabilities allow only the window controls the banner
  needs. Every command argument is validated in Rust. There is no shell access and no arbitrary
  file access. The only links Shuusshy opens are fixed (VB-CABLE site, GitHub Releases).
- **Signed updates.**
  - Updates are verified against a public key built into the app before they are installed;
    invalid or tampered downloads are rejected.
  - The signing key is kept outside the repository (a GitHub Actions secret for CI).
- **Signed release metadata.** Release and expiry dates are signed and verified offline.
- **Least privilege.**
  - Shuusshy installs per user without administrator rights.
  - The only elevated action is the optional rename of the virtual mic. That helper accepts
    only two fixed names for VB-CABLE's own endpoint.
- **Memory safety.** The backend is Rust, and the audio paths do not allocate or lock in
  real-time callbacks.

## Known limitations

- Release binaries are not code-signed with an Authenticode certificate yet, so Windows
  SmartScreen may warn.
- Offline license keys and the release expiry are best-effort: whoever can modify the program
  or the system clock can bypass them. This is documented on purpose; Shuusshy uses no
  invasive anti-tampering.

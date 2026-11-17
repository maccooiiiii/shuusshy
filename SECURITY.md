# Security

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

- GitHub: **Security → Report a vulnerability** on this repository (private advisory), or
- email **shush_support@ianmusa.com** with "Security" in the subject.

Include the version (Settings → About), steps to reproduce and impact. You will get an answer as
soon as possible. Please allow time for a fix before publishing details.

## Supported versions

Each official release is supported for its validity period of two years from its release date
(see [docs/RELEASES.md](docs/RELEASES.md)). Fixes ship in new releases.

## What is public, and what is not

This repository is public, so it contains nothing secret.

- **Public on purpose:** the source code, the website, the license *verification* key
  (`src-tauri/src/license.rs`) and the updater *public* key (`tauri.conf.json`). These keys can
  only check signatures; they cannot create Pro keys or sign updates.
- **Never in the repository:** the license signing key, the release signing key, the updater
  private key and the list of issued Pro keys. They stay offline with the maintainer.
  - `.gitignore` blocks key, database, certificate and `.env` files.
  - GitHub secret scanning with push protection is on.
- **CI secrets:**
  - The signing secrets are stored only in the `release` environment, which accepts `v*` tags
    only. Only the maintainer can create, move or delete `v*` tags.
  - Workflows get read-only tokens by default, and checkouts don't keep the token.
  - Every action is pinned to an exact commit, and only GitHub's actions plus two named
    third-party actions are allowed.
  - The signing build never uses a build cache.
- **Branch protection:** `main` cannot be force-pushed or deleted.
- **Dependencies:** Dependabot alerts, security updates and weekly update checks (npm, Cargo,
  GitHub Actions).

## How Shuusshy protects you

- **Local processing.** No account; Shuusshy's only network use is the optional update check.
  The WebView2 runtime may contact Microsoft on its own ([PRIVACY.md](PRIVACY.md)).
- **Restricted app permissions.**
  - Tauri capabilities allow only the window controls the banner needs.
  - Every command argument is validated in Rust, and a strict content security policy applies.
  - There is no shell access and no arbitrary file access.
  - The only links Shuusshy opens are fixed `https://` addresses (VB-CABLE site, GitHub Releases).
- **Signed updates.**
  - Updates are verified against a public key built into the app before they are installed;
    invalid or tampered downloads are rejected.
  - The signing key is kept outside the repository (a GitHub Actions environment secret for CI).
- **Signed release metadata.** Release and expiry dates are signed and verified offline.
- **Offline Pro keys.** Keys are Ed25519 signatures checked offline (strict verification). No
  license server is contacted.
- **Least privilege.**
  - Shuusshy installs per user without administrator rights.
  - Only two actions ask for administrator rights, and only when you start them:
    - the optional rename of the virtual mic, whose helper accepts only two fixed names for
      VB-CABLE's own endpoint;
    - VB-Audio's own VB-CABLE setup, started from a fixed path in the install folder.
- **Memory safety.** The backend is Rust, and the audio paths do not allocate or lock in
  real-time callbacks.

## Known limitations

- Release binaries are not code-signed with an Authenticode certificate yet, so Windows
  SmartScreen may warn.
- Offline license keys and the release expiry are best-effort: whoever can modify the program
  or the system clock can bypass them. This is documented on purpose; Shuusshy uses no
  invasive anti-tampering.

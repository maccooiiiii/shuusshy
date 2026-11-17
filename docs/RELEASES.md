# Releases, updates and the two-year validity period

## Versions

Shuusshy uses semantic versioning:
- `1.0.1`: bug fixes;
- `1.1.0`: new compatible features;
- `2.0.0`: major changes;
- `X.Y.Z-beta.N`: beta channel (marked as a pre-release on GitHub).

Every release has release notes: its `## [X.Y.Z]` section in `CHANGELOG.md`.

## Validity period (spec Part 10)

Each official release is valid for **two calendar years from its release date** (29 February
becomes 28 February). Example: released 2027-01-01, valid until 2029-01-01.

| When | What the user sees |
|---|---|
| More than 30 days left | Nothing. Settings → Application shows "valid until …" |
| 30 days or less | A discreet reminder line on the banner |
| 7 days or less | A prominent "Update soon" card |
| Expired | "This version has expired". Noise cancellation and the virtual microphone stop; settings are kept; "Get the latest version" and "Export settings" stay available. Installing a newer release restores everything |

How it works:
- The release workflow signs `version | channel | release date` with the **release key**
  (Ed25519) and compiles the signature into the build (`src-tauri/src/release.rs`).
- The app verifies it offline against the built-in public key.
- Builds without valid metadata (local or development builds) say **"Unofficial build"** and
  do not expire.

**Limits, disclosed on purpose:**
- The check uses the PC's clock.
- Anyone who can rebuild or modify the program can remove it. Shuusshy uses no
  anti-tampering, hidden persistence or clock manipulation.
- Users see the policy before installing (installer terms page, `src-tauri/installer/TERMS.txt`),
  in the README and here.
- **Have the LICENSE and these terms reviewed by a lawyer before the first public release.**

## Update system (spec Part 13)

- **Check:**
  - The app downloads `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.
  - It does this on start-up when "Check for updates automatically" is on (at most daily), and
    on "Check for updates".
  - The repository comes from the build (`SHUUSSHY_GITHUB_REPO`); without it, no network
    connection is ever made.
- **Notify:** "Shuusshy X is available" appears on the banner. Settings shows the installed and
  available versions and the release notes.
- **Download:** a progress bar shows the download. "Download updates automatically" is optional
  and off by default. A failed or interrupted download leaves nothing behind; try again.
- **Verify:** the Tauri updater checks the installer's signature against the **updater public
  key** in `tauri.conf.json` and rejects anything else.
- **Install:** always on the user's click ("Restart and update"). Audio stops cleanly, the
  installer runs in passive mode, and settings, the Pro key and Start with Windows are kept.
- **Portable edition:** updates are not installed in place. The app links to the Releases page;
  replace `shuusshy.exe` and keep `data\`.
- **Expired versions:** the update check and the Releases link keep working, because updates
  are signature-verified independently of the expiry.

## Keys and secrets

| Key | Where it lives | Used for |
|---|---|---|
| `release-signing.key` | `%USERPROFILE%\.shuusshy\`, and the `release` environment secret `SHUUSSHY_RELEASE_SIGNING_KEY` (hex contents) | Signing release metadata (validity period) |
| `updater.key` (no password) | `%USERPROFILE%\.shuusshy\`, and the `release` environment secret `TAURI_SIGNING_PRIVATE_KEY` (file contents) | Signing update installers |
| `license-signing.key` | `%USERPROFILE%\.shuusshy\` only (never in CI) | Issuing Pro keys |

The list of issued Pro keys (`issued-keys.txt`) lives there too. Back all four up offline
(for example an encrypted USB drive). Never commit them: `.gitignore` blocks `*.key`,
`issued-keys*` and database files, and GitHub push protection is on (see SECURITY.md). Replacing a key requires
a new public key in the app:
- release key: `release.rs`;
- updater key: `tauri.conf.json`;
- license key: `license.rs`.

After that, older builds can no longer verify the new signatures.

## Making a release

1. **One-time setup:**
   - Create the GitHub repository.
   - `git remote add origin https://github.com/<owner>/shuusshy.git`
   - Create the `release` environment (Settings → Environments), allow only `v*` tags to
     deploy to it, and add the two secrets above as environment secrets, not repository
     secrets.
2. Set the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
3. Rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD` and add a new empty
   Unreleased section.
4. Commit, then tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`.
5. The **Release** workflow:
   - runs every check;
   - builds the signed, dated installer, portable ZIP, `latest.json` and `SHA256SUMS.txt`;
   - creates a **draft** GitHub release.
6. Review the draft (try the installer), then publish it. Installed copies see the update at
   their next check.

The CI workflow builds unofficial artifacts on every push to `main` and on pull requests, for
build validation.

fn main() {
    // Release metadata and the repository address are baked in by the release workflow
    // (see src/release.rs); rebuild when they change.
    for var in [
        "SHUUSSHY_RELEASE_CHANNEL",
        "SHUUSSHY_RELEASE_DATE",
        "SHUUSSHY_RELEASE_SIG",
        "SHUUSSHY_GITHUB_REPO",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    tauri_build::build()
}

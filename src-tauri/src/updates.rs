//! Update system (spec Part 13): check GitHub Releases for a newer version, download it with
//! progress, verify its signature (the updater plugin rejects anything not signed by the key
//! in tauri.conf.json), and install it on request. Settings live in %APPDATA% and survive.
//!
//! Network use is limited to the release file (`latest.json`) and the update installer, only
//! from the repository baked into the build, and only when the user allows update checks.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::AppState;

pub const UPDATE_AVAILABLE: &str = "update-available";
pub const UPDATE_PROGRESS: &str = "update-progress";
const DAY_SECS: i64 = 86_400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub date: Option<String>,
    /// Release notes (Markdown text from the GitHub release).
    pub notes: Option<String>,
    /// Downloaded and verified, ready to install.
    pub ready: bool,
    /// Portable edition: updates are installed by replacing the folder from the Releases
    /// page, never by the installer (which would add a second, installed copy).
    pub portable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Default)]
pub struct Updates {
    /// The update found by the last check, and its downloaded bytes once fetched.
    pending: Mutex<Option<(Update, Option<Vec<u8>>)>>,
    busy: AtomicBool,
}

/// `latest.json` of the official repository, if this build knows it.
pub fn endpoint() -> Option<Url> {
    let repo = crate::release::GITHUB_REPO.filter(|r| crate::release::valid_repo(r))?;
    format!("https://github.com/{repo}/releases/latest/download/latest.json")
        .parse()
        .ok()
}

fn info(app: &AppHandle, u: &Update, ready: bool) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        current_version: app.package_info().version.to_string(),
        date: u.date.map(|d| d.date().to_string()),
        notes: u.body.clone(),
        ready,
        portable: crate::install::portable_data_dir().is_some(),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn readable(e: impl std::fmt::Display) -> String {
    let e = e.to_string();
    if e.contains("signature") {
        format!(
            "The update was rejected because its signature is not valid ({e}). Nothing was installed."
        )
    } else if e.contains("dns")
        || e.contains("connect")
        || e.contains("timed out")
        || e.contains("network")
    {
        "Could not reach GitHub. Check your internet connection and try again.".into()
    } else {
        format!("Update failed: {e}")
    }
}

/// Asks GitHub for a newer release. `Ok(None)` = up to date.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let url =
        endpoint().ok_or("Update checks are not set up in this build (no repository address).")?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(readable)?
        .build()
        .map_err(readable)?;
    let found = updater.check().await.map_err(readable)?;
    if let Some(state) = app.try_state::<AppState>()
        && let Ok(mut s) = state.settings.lock()
    {
        s.app.last_update_check = now_secs();
        if let Ok(mut store) = state.store.lock() {
            let _ = store.save(&s);
        }
    }
    let updates = app.state::<Updates>();
    let mut pending = updates.pending.lock().unwrap_or_else(|p| p.into_inner());
    Ok(match found {
        Some(u) => {
            let i = info(app, &u, false);
            *pending = Some((u, None));
            Some(i)
        }
        None => {
            *pending = None;
            None
        }
    })
}

/// Downloads the update found by the last check, reporting progress. Interrupted downloads
/// leave nothing behind and can simply be retried.
pub async fn download(app: &AppHandle) -> Result<UpdateInfo, String> {
    if crate::install::portable_data_dir().is_some() {
        return Err("This is the portable edition: download the new ZIP from the Releases page and replace shuusshy.exe (keep the data folder).".into());
    }
    let updates = app.state::<Updates>();
    if updates.busy.swap(true, SeqCst) {
        return Err("A download is already running.".into());
    }
    let result = async {
        let update = {
            let pending = updates.pending.lock().unwrap_or_else(|p| p.into_inner());
            pending.as_ref().map(|(u, _)| u.clone())
        }
        .ok_or("Check for updates first.")?;
        let handle = app.clone();
        let mut downloaded = 0u64;
        let bytes = update
            .download(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    let _ = handle.emit(UPDATE_PROGRESS, Progress { downloaded, total });
                },
                || {},
            )
            .await
            .map_err(readable)?;
        let i = info(app, &update, true);
        *updates.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some((update, Some(bytes)));
        Ok(i)
    }
    .await;
    updates.busy.store(false, SeqCst);
    result
}

/// Installs the downloaded update. The installer closes Shuusshy, keeps the settings and
/// starts the new version.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let (update, bytes) = updates
        .pending
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .ok_or("Download the update first.")?;
    let bytes = bytes.ok_or("Download the update first.")?;
    // Stop audio cleanly so the virtual microphone goes silent instead of glitching.
    if let Some(a) = app.try_state::<crate::commands::Audio>() {
        a.0.shutdown();
    }
    update.install(bytes).map_err(readable)?;
    app.restart();
}

/// At start-up: if allowed and the last check is a day old, check quietly; announce a found
/// update to the banner and download it in the background if the user chose that.
pub fn spawn_startup_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio_sleep(15).await;
        let (allowed, auto, last) = match app.try_state::<AppState>() {
            Some(s) => match s.settings.lock() {
                Ok(s) => (
                    s.app.check_updates,
                    s.app.auto_download_updates,
                    s.app.last_update_check,
                ),
                Err(_) => return,
            },
            None => return,
        };
        if !allowed || endpoint().is_none() || now_secs() - last < DAY_SECS {
            return;
        }
        if let Ok(Some(i)) = check(&app).await {
            let portable = i.portable;
            let _ = app.emit(UPDATE_AVAILABLE, i);
            if auto
                && !portable
                && let Ok(i) = download(&app).await
            {
                let _ = app.emit(UPDATE_AVAILABLE, i);
            }
        }
    });
}

async fn tokio_sleep(secs: u64) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let _ = tx.send(());
    });
    let _ = tauri::async_runtime::spawn_blocking(move || rx.recv()).await;
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_endpoint_without_repository() {
        // Test builds have no SHUUSSHY_GITHUB_REPO, so update checks stay off (no network).
        if crate::release::GITHUB_REPO.is_none() {
            assert!(super::endpoint().is_none());
        }
    }
}

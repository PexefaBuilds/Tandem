//! Updates from GitHub Releases through tauri-plugin-updater. Every package is checked against the public key
//! in tauri.conf.json before it is installed. "Update now" downloads, installs over the current copy (no uninstall)
//! and restarts Tandem.
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const ENDPOINT: &str = "https://github.com/PexefaBuilds/Tandem/releases/latest/download/latest.json";

#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    newer: bool,
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let url = ENDPOINT.parse().map_err(|_| "Bad update address.".to_string())?;
    let u = app.updater_builder().endpoints(vec![url]).and_then(|b| b.build()).map_err(|_| "Couldn't start the update check.".to_string())?;
    let found = match u.check().await {
        Ok(f) => f,
        // The latest release has no signed manifest (an unsigned build).
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => None,
        Err(_) => return Err("Couldn't check for updates. Check your connection.".into()),
    };
    let info = UpdateInfo { latest: found.as_ref().map(|u| u.version.clone()).unwrap_or_else(|| current.clone()), newer: found.is_some(), current };
    *app.state::<Pending>().0.lock().unwrap_or_else(|e| e.into_inner()) = found;
    Ok(info)
}

/// Downloads the update found by the last check, installs it and restarts.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let u = app.state::<Pending>().0.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("Check for updates first.")?;
    let bytes = u.download(|_, _| {}, || {}).await.map_err(|_| "The download failed. Try again.".to_string())?;
    u.install(&bytes).map_err(|_| "Couldn't install the update.".to_string())?;
    app.restart()
}

//! Sign-in for a profile. Only the app's default slot receives the OS sign-in callback, so for Claude we
//! set the Main instance's data aside, let the user sign in to a blank Claude, adopt that data into the
//! profile, and put the Main instance's data back. See docs/EXISTING-APP.md section 4.
use crate::engine::{self, App};
use crate::import;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Serialize)]
pub struct Status {
    /// idle | waiting | detected | ready | failed | cancelled
    pub phase: &'static str,
    pub message: String,
}

static STATUS: Mutex<Status> = Mutex::new(Status { phase: "idle", message: String::new() });
static CANCEL: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn set(phase: &'static str, message: &str) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Status { phase, message: message.into() };
}
pub fn status() -> Status {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Claude's default slot: the MSIX-virtualised roaming folder when the package folder exists.
pub fn default_slot_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME").map(|h| Path::new(&h).join("Library/Application Support/Claude"));
    #[cfg(not(target_os = "macos"))]
    windows_slot_dir()
}

#[cfg(not(target_os = "macos"))]
fn windows_slot_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    // Newer Claude versions keep the data here, outside the package.
    let newer = Path::new(&local).join("Claude-Data").join("Claude_pzs8sxrjxfjjc".to_lowercase());
    if newer.join("config.json").is_file() {
        return Some(newer);
    }
    let pkg =Path::new(&local).join("Packages").join("Claude_pzs8sxrjxfjjc");
    if pkg.is_dir() {
        return Some(pkg.join("LocalCache").join("Roaming").join("Claude"));
    }
    Some(Path::new(&std::env::var_os("APPDATA")?).join("Claude"))
}

fn backup_of(slot: &Path) -> PathBuf {
    slot.with_file_name(format!("{}.tandem-backup", slot.file_name().and_then(|n| n.to_str()).unwrap_or("Claude")))
}

pub fn has_orphan_backup() -> bool {
    default_slot_dir().is_some_and(|s| backup_of(&s).is_dir())
}

/// Rename with retries (locks linger after a kill); fall back to copy and delete across volumes.
fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        return Err(format!("Destination already exists: {}", dst.display()));
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match std::fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(17) => {
                import::copy_all(src, dst).map_err(|e| e.to_string())?;
                return engine::remove_dir_retry(src);
            }
            Err(e) if Instant::now() >= deadline => return Err(e.to_string()),
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

pub fn restore() -> Result<(), String> {
    let slot = default_slot_dir().ok_or("Not available")?;
    let backup = backup_of(&slot);
    if !backup.is_dir() {
        return Ok(());
    }
    if slot.is_dir() {
        engine::remove_dir_retry(&slot)?; // discards whatever the sign-in left behind
    }
    move_dir(&backup, &slot)
}

pub fn discard_backup() -> Result<(), String> {
    let slot = default_slot_dir().ok_or("Not available")?;
    engine::remove_dir_retry(&backup_of(&slot))
}

fn config_has_oauth(slot: &Path) -> bool {
    std::fs::read_to_string(slot.join("config.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .is_some_and(|v| ["oauth:tokenCacheV2", "oauth:tokenCache"].iter().any(|k| v.get(k).is_some_and(|x| !x.is_null() && x != "" && x != false)))
}

/// stat() only: the cookie database is locked while the app runs.
fn cookie_signature(slot: &Path) -> Vec<(bool, u64, Option<SystemTime>)> {
    ["Cookies", "Cookies-journal", "Cookies-wal"]
        .iter()
        .map(|f| match std::fs::metadata(slot.join("Network").join(f)) {
            Ok(m) => (true, m.len(), m.modified().ok()),
            Err(_) => (false, 0, None),
        })
        .collect()
}

/// Whether a profile folder holds a sign-in (cheap file checks; real account details arrive with usage in M3).
pub fn signed_in(app: &App, folder: &Path) -> bool {
    if app.key == "claude" { config_has_oauth(folder) } else { folder.join(".codex").join("auth.json").is_file() }
}

fn session_captured(slot: &Path) -> bool {
    config_has_oauth(slot) && std::fs::metadata(slot.join("Network").join("Cookies")).is_ok_and(|m| m.len() > 0)
}

fn wait_gone(how: impl Fn() -> Vec<u32>, secs: u64) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline && !how().is_empty() {
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn main_pids(app: &App) -> Vec<u32> {
    engine::main_pids_from(&engine::processes(app), app)
}

pub fn start(app: App, name: String, folder: PathBuf, profiles: PathBuf) -> Result<(), String> {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return Err("A sign-in is already in progress.".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    set("waiting", "Starting…");
    std::thread::spawn(move || {
        let out = if app.key == "claude" { run_claude(&app, &folder, &profiles) } else { run_codex(&app, &folder) };
        match out {
            Ok(true) => set("ready", &name),
            Ok(false) => set("cancelled", "Sign-in was cancelled."),
            Err(e) => set("failed", &e),
        }
        ACTIVE.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn run_codex(app: &App, folder: &Path) -> Result<bool, String> {
    let auth = folder.join(".codex").join("auth.json");
    engine::launch(app, folder)?;
    set("waiting", "Waiting for you to sign in…");
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        if CANCEL.load(Ordering::SeqCst) {
            return Ok(false);
        }
        if auth.is_file() {
            set("detected", "Sign-in detected.");
            std::thread::sleep(Duration::from_millis(1200));
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Timed out waiting for sign-in.".into())
}

enum Outcome {
    Ready,
    Cancelled,
}

fn run_claude(app: &App, folder: &Path, profiles: &Path) -> Result<bool, String> {
    let slot = default_slot_dir().ok_or("Claude's data folder wasn't found.")?;
    let backup = backup_of(&slot);
    if backup.is_dir() {
        return Err("A previous sign-in left your main session set aside. Restart Tandem to restore it.".into());
    }
    let main_was_running = !main_pids(app).is_empty();
    // Close the target profile first if it is open.
    let running = engine::running(app, profiles).profiles;
    let fname = folder.file_name().unwrap_or_default().to_string_lossy().into_owned();
    if let Some((_, pids)) = running.iter().find(|(k, _)| k.eq_ignore_ascii_case(&fname)) {
        engine::kill_pids(pids);
    }
    set("waiting", "Closing Claude and setting your session aside…");
    engine::kill_pids(&main_pids(app));
    wait_gone(|| main_pids(app), 20);

    let mut stashed = false;
    let mut adopted = false;
    let result = (|| -> Result<Outcome, String> {
        if slot.is_dir() {
            move_dir(&slot, &backup)?;
            stashed = true;
        }
        engine::launch_main(app)?;
        set("waiting", "Waiting for you to sign in…");
        let outcome = detect(&slot, app)?;
        if let Outcome::Cancelled = outcome {
            return Ok(outcome);
        }
        engine::kill_pids(&main_pids(app));
        wait_gone(|| main_pids(app), 15);
        if !session_captured(&slot) {
            return Err("Couldn't capture the full session, so nothing was copied. Your main session is untouched. Try again.".into());
        }
        if folder.exists() {
            engine::remove_dir_retry(folder)?; // adopting replaces the profile entirely
        }
        std::fs::create_dir_all(profiles).map_err(|e| e.to_string())?;
        move_dir(&slot, folder)?;
        adopted = true;
        Ok(Outcome::Ready)
    })();

    // Always put the main session back.
    engine::kill_pids(&main_pids(app));
    wait_gone(|| main_pids(app), 15);
    if stashed && backup.is_dir() {
        if let Err(e) = restore() {
            return Err(format!("Could not restore your main session: {e}. It is safe at {}. Restart Tandem to recover it.", backup.display()));
        }
    }
    if main_was_running && !has_orphan_backup() {
        let _ = engine::launch_main(app);
    }
    match result? {
        Outcome::Ready if adopted => Ok(true),
        Outcome::Cancelled => Ok(false),
        Outcome::Ready => Err("Sign-in didn't finish.".into()),
    }
}

/// Ready when the OAuth key and a cookie-store change both happened, the cookie change no more than 2 s before.
fn detect(slot: &Path, app: &App) -> Result<Outcome, String> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut reference = cookie_signature(slot);
    let (mut login_at, mut cookie_at): (Option<Instant>, Option<Instant>) = (None, None);
    let (mut seen_running, mut gone_checks) = (false, 0);
    let mut last_proc_check = Instant::now();
    while Instant::now() < deadline {
        if CANCEL.load(Ordering::SeqCst) {
            return Ok(Outcome::Cancelled);
        }
        let sig = cookie_signature(slot);
        if sig != reference {
            reference = sig;
            cookie_at = Some(Instant::now());
        }
        if login_at.is_none() && config_has_oauth(slot) {
            login_at = Some(Instant::now());
            set("detected", "Sign-in detected. Saving the session…");
        }
        if let (Some(l), Some(c)) = (login_at, cookie_at) {
            if c + Duration::from_secs(2) >= l {
                std::thread::sleep(Duration::from_millis(1200));
                return Ok(Outcome::Ready);
            }
        }
        // The window was closed before sign-in finished.
        if login_at.is_none() && last_proc_check.elapsed() > Duration::from_secs(3) {
            last_proc_check = Instant::now();
            if main_pids(app).is_empty() {
                gone_checks += 1;
                if seen_running && gone_checks >= 2 {
                    return Err("The Claude window was closed before you signed in. Nothing was changed.".into());
                }
            } else {
                seen_running = true;
                gone_checks = 0;
            }
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Err("Timed out waiting for sign-in. Nothing was changed.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_detection_reads_config() {
        let d = std::env::temp_dir().join(format!("tandem-signin-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(!config_has_oauth(&d));
        std::fs::write(d.join("config.json"), r#"{"oauth:tokenCacheV2": null}"#).unwrap();
        assert!(!config_has_oauth(&d));
        std::fs::write(d.join("config.json"), r#"{"oauth:tokenCacheV2": "abc"}"#).unwrap();
        assert!(config_has_oauth(&d));
        std::fs::write(d.join("config.json"), "not json").unwrap();
        assert!(!config_has_oauth(&d));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn move_dir_moves_and_refuses_to_overwrite() {
        let d = std::env::temp_dir().join(format!("tandem-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a").join("f"), "x").unwrap();
        move_dir(&d.join("a"), &d.join("b")).unwrap();
        assert!(d.join("b").join("f").is_file() && !d.join("a").exists());
        std::fs::create_dir_all(d.join("c")).unwrap();
        assert!(move_dir(&d.join("c"), &d.join("b")).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn backup_name_sits_beside_the_slot() {
        assert_eq!(backup_of(Path::new(r"C:\x\Claude")), PathBuf::from(r"C:\x\Claude.tandem-backup"));
    }
}

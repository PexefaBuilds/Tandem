//! Desktop shortcuts (.lnk) that run `Tandem.exe --launch <app> <profile>`.
use crate::engine::App;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(windows)]
fn desktop() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    // The Desktop can be redirected (OneDrive), so ask Windows rather than guessing.
    let out = Command::new("powershell").args(["-NoProfile", "-Command", "[Environment]::GetFolderPath('Desktop')"]).creation_flags(0x0800_0000).stdin(Stdio::null()).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then(|| s.into())
}
#[cfg(not(windows))]
fn desktop() -> Option<PathBuf> {
    None
}

fn file(app: &App, name: &str) -> Option<PathBuf> {
    static DESKTOP: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DESKTOP.get_or_init(desktop).clone().map(|d| d.join(format!("{name} ({}).lnk", app.display)))
}

pub fn exists(app: &App, name: &str) -> bool {
    file(app, name).is_some_and(|f| f.exists())
}

pub fn remove(app: &App, name: &str) {
    if let Some(f) = file(app, name) {
        let _ = std::fs::remove_file(f);
    }
}

pub fn create(app: &App, name: &str) -> Result<(), String> {
    let f = file(app, name).ok_or("Desktop shortcuts need Windows for now.")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let q = |s: &str| s.replace('\'', "''");
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath='{}'; $s.Arguments='--launch {} \"{}\"'; $s.IconLocation='{}'; $s.Save()",
        q(&f.to_string_lossy()),
        q(&exe.to_string_lossy()),
        app.key,
        q(name),
        q(&exe.to_string_lossy())
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let ok = Command::new("powershell").args(["-NoProfile", "-Command", &script]).creation_flags(0x0800_0000).stdin(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        return if ok && f.exists() { Ok(()) } else { Err("Couldn't create the shortcut.".into()) };
    }
    #[cfg(not(windows))]
    {
        let _ = script;
        Err("Desktop shortcuts need Windows for now.".into())
    }
}

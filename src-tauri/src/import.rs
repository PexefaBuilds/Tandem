//! Bring existing profile folders over by copying them. The originals are never changed.
use std::fs;
use std::path::{Path, PathBuf};

/// Regenerable folders that make a profile many GB for nothing (caches, logs, downloaded VM and CLI bundles).
const SKIP_DIRS: [&str; 13] = [
    "Cache", "Code Cache", "GPUCache", "DawnGraphiteCache", "DawnWebGPUCache", "GrShaderCache", "ShaderCache", "GPUPersistentCache", "Crashpad", "logs", "vm_bundles", "claude-code", "VideoDecodeStats",
];
const SKIP_FILES: [&str; 4] = ["lockfile", "SingletonLock", "SingletonCookie", "SingletonSocket"];

fn has_oauth(dir: &Path) -> bool {
    fs::read_to_string(dir.join("config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().map(|o| o.keys().any(|k| k.starts_with("oauth:"))))
        .unwrap_or(false)
}

/// "claude" or "codex" when `dir` looks like a user-data folder of that app. Claude is recognised by its own
/// files, not by generic Chromium ones, so Chrome and other Electron apps are never listed.
pub fn kind_of(dir: &Path) -> Option<&'static str> {
    if dir.join(".codex").is_dir() {
        Some("codex")
    } else if dir.join("claude_desktop_config.json").is_file() || dir.join("ant-did").is_file() || has_oauth(dir) {
        Some("claude")
    } else {
        None
    }
}

/// The folder itself, or each of its sub-folders, that looks like a profile.
pub fn scan(path: &Path) -> Vec<(PathBuf, &'static str)> {
    if let Some(k) = kind_of(path) {
        return vec![(path.to_path_buf(), k)];
    }
    let Ok(rd) = fs::read_dir(path) else { return vec![] };
    let mut found: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).filter_map(|p| kind_of(&p).map(|k| (p, k))).collect();
    found.sort_by_key(|(p, _)| p.file_name().map(|n| n.to_ascii_lowercase()));
    found
}

pub fn has_sign_in_data(dir: &Path) -> bool {
    dir.join("Network").join("Cookies").is_file() || dir.join(".codex").join("auth.json").is_file()
}

/// Full copy with nothing skipped (used for cross-volume moves).
pub fn copy_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    copy_dir(src, dst, false)
}

pub fn copy_profile(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    copy_dir(src, dst, true).map_err(|e| match e.raw_os_error() {
        Some(32) => "A file is in use. Close the profile and try again.".to_string(),
        _ => format!("Copy failed: {e}"),
    })
}

fn copy_dir(src: &Path, dst: &Path, top: bool) -> std::io::Result<()> {
    for e in fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        let name_s = name.to_string_lossy();
        let ty = e.file_type()?;
        if top && ty.is_dir() && SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(&name_s)) {
            continue;
        }
        if top && SKIP_FILES.iter().any(|s| s.eq_ignore_ascii_case(&name_s)) {
            continue;
        }
        let to = dst.join(&name);
        if ty.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir(&e.path(), &to, false)?;
        } else if ty.is_file() {
            fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

const SKIP_WALK: [&str; 14] = [
    "windows", "program files", "program files (x86)", "programdata", "appdata", "node_modules", "$recycle.bin", "system volume information", "target", "library", "steam", "steamlibrary", "onedrive", "dropbox",
];

/// Where profiles usually live: the app's default folder, plus any folder of profiles within a few levels of
/// the user's folders and the drive roots. Each hit is (path, app, is_default).
pub type Hit = (PathBuf, &'static str, bool);

/// Results go into `out` as they are found, so a caller that stops waiting still gets what was found so far.
pub fn scan_computer(default_slot: Option<PathBuf>, ours: &Path, out: &std::sync::Mutex<Vec<Hit>>) {
    let push = |h: Hit| out.lock().unwrap_or_else(|e| e.into_inner()).push(h);
    if let Some(d) = default_slot {
        if let Some(k) = kind_of(&d) {
            push((d, k, true));
        }
    }
    let mut roots: Vec<PathBuf> = vec![];
    if let Some(h) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        roots.push(h.into());
    }
    if cfg!(windows) {
        roots.extend(('C'..='Z').map(|c| PathBuf::from(format!("{c}:\\"))).filter(|p| p.is_dir()));
    }
    // Breadth-first with a time budget: the usual places come first, and a huge or network drive can't stall it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
    let mut queue: std::collections::VecDeque<(PathBuf, u32)> = roots.into_iter().map(|r| (r, 0)).collect();
    let mut seen = std::collections::HashSet::new();
    while let Some((dir, depth)) = queue.pop_front() {
        if std::time::Instant::now() > deadline {
            break;
        }
        if depth > 4 || dir.starts_with(ours) {
            continue;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if !e.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if SKIP_WALK.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            if let Some(k) = kind_of(&p) {
                if seen.insert(p.clone()) {
                    push((p, k, false));
                }
            } else {
                queue.push_back((p, depth + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("tandem-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn scan_finds_single_and_parent_folders() {
        let root = tmp("scan");
        fs::create_dir_all(root.join("A")).unwrap();
        fs::write(root.join("A").join("claude_desktop_config.json"), "{}").unwrap();
        fs::create_dir_all(root.join("B").join(".codex")).unwrap();
        fs::create_dir_all(root.join("C")).unwrap(); // not a profile
        let got = scan(&root);
        assert_eq!(got.iter().map(|(p, k)| (p.file_name().unwrap().to_string_lossy().into_owned(), *k)).collect::<Vec<_>>(), vec![("A".to_string(), "claude"), ("B".to_string(), "codex")]);
        assert_eq!(scan(&root.join("A")).len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generic_chromium_folders_are_not_profiles() {
        let d = tmp("chrome");
        fs::create_dir_all(d.join("Network")).unwrap();
        fs::write(d.join("Local State"), "x").unwrap();
        fs::write(d.join("Network").join("Cookies"), "x").unwrap();
        assert_eq!(kind_of(&d), None);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn copy_keeps_sign_in_data_and_skips_caches() {
        let src = tmp("src");
        fs::create_dir_all(src.join("Network")).unwrap();
        fs::write(src.join("Network").join("Cookies"), "cookie").unwrap();
        fs::write(src.join("Local State"), "ls").unwrap();
        fs::write(src.join("ant-did"), "id").unwrap();
        fs::create_dir_all(src.join("Cache")).unwrap();
        fs::write(src.join("Cache").join("big"), "junk").unwrap();
        fs::write(src.join("lockfile"), "").unwrap();
        let dst = tmp("dst").join("out");
        copy_profile(&src, &dst).unwrap();
        assert_eq!(fs::read_to_string(dst.join("Network").join("Cookies")).unwrap(), "cookie");
        assert!(dst.join("Local State").is_file());
        assert!(!dst.join("Cache").exists() && !dst.join("lockfile").exists());
        assert!(src.join("Cache").join("big").is_file(), "original untouched");
        let _ = fs::remove_dir_all(src);
        let _ = fs::remove_dir_all(dst.parent().unwrap());
    }
}

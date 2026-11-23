//! Desktop-app engine: locate the MSIX install, detect running profiles, launch and close.
//! Behaviour follows docs/EXISTING-APP.md (sections 1, 3, 4.2 and the gotchas).
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy)]
pub struct App {
    pub key: &'static str,
    pub display: &'static str,
    pub package_filter: &'static str,
    pub package_prefix: &'static str,
    pub exe_name: &'static str,
    /// macOS bundle name without `.app`.
    pub mac_bundle: &'static str,
    pub profiles_dirname: &'static str,
    /// Extra env vars: (name, subfolder of the profile folder).
    pub env_overrides: &'static [(&'static str, &'static str)],
}

pub const CLAUDE: App = App {
    key: "claude",
    display: "Claude",
    package_filter: "*Claude*",
    package_prefix: "Claude_",
    exe_name: "claude.exe",
    mac_bundle: "Claude",
    profiles_dirname: "ClaudeProfiles",
    env_overrides: &[],
};
pub const CODEX: App = App {
    key: "codex",
    display: "Codex",
    package_filter: "*Codex*",
    package_prefix: "OpenAI.Codex_",
    exe_name: "ChatGPT.exe",
    mac_bundle: "Codex",
    profiles_dirname: "CodexProfiles",
    // Codex isolation needs both of these on top of --user-data-dir.
    env_overrides: &[("CODEX_HOME", ".codex"), ("CODEX_ELECTRON_USER_DATA_PATH", "electron")],
};

/// Terminal tools have no MSIX package or process to track; `package_filter` is empty for them.
pub const CLAUDE_CODE: App = App { key: "cc", display: "Claude Code", package_filter: "", package_prefix: "", exe_name: "claude", mac_bundle: "", profiles_dirname: "ClaudeCodeProfiles", env_overrides: &[] };
pub const CODEX_CLI: App = App { key: "cx", display: "Codex CLI", package_filter: "", package_prefix: "", exe_name: "codex", mac_bundle: "", profiles_dirname: "CodexCliProfiles", env_overrides: &[] };

impl App {
    pub fn is_terminal(&self) -> bool {
        self.package_filter.is_empty()
    }
}

pub fn app_by_key(key: &str) -> Option<App> {
    [CLAUDE, CODEX, CLAUDE_CODE, CODEX_CLI].into_iter().find(|a| a.key == key)
}

#[cfg(windows)]
fn hide(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000) // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn hide(cmd: &mut Command) -> &mut Command {
    cmd
}

/// Runs a script with no window and a 20 s limit; a script that overruns is killed and reaped.
pub fn powershell(script: &str) -> String {
    use std::io::Read;
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    hide(&mut c);
    let Ok(mut child) = c.spawn() else { return String::new() };
    let Some(mut out) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return String::new();
    };
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned()
}

/// The MSIX path changes on every update, so it is resolved lazily and never stored on disk.
pub fn find_exe(app: &App) -> Option<PathBuf> {
    static CACHE: std::sync::Mutex<Vec<(&str, std::time::Instant, Option<PathBuf>)>> = std::sync::Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    // A found path stays valid until the file is gone (an MSIX update moves it), so the slow
    // Get-AppxPackage runs only on a miss; a "not installed" answer is retried after 60 s.
    if let Some((_, at, v)) = cache.iter().find(|(k, _, _)| *k == app.key) {
        let fresh = match v {
            Some(p) => p.is_file(),
            None => at.elapsed().as_secs() < 60,
        };
        if fresh {
            return v.clone();
        }
    }
    let found = resolve_exe(app);
    cache.retain(|(k, _, _)| *k != app.key);
    cache.push((app.key, std::time::Instant::now(), found.clone()));
    found
}

fn resolve_exe(app: &App) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::find_bundle(app.mac_bundle);
    #[cfg(not(target_os = "macos"))]
    resolve_msix(app)
}

#[cfg(not(target_os = "macos"))]
fn resolve_msix(app: &App) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let out = powershell(&format!("(Get-AppxPackage -Name {}).InstallLocation", app.package_filter));
    out.lines()
        .map(|l| Path::new(l.trim()).join("app").join(app.exe_name))
        .find(|p| p.is_file())
}

/// `Claude_1.2.3.0_x64__hash` -> `1.2.3.0`
pub fn app_version(exe: &Path) -> String {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::bundle_version(exe);
    #[cfg(not(target_os = "macos"))]
    msix_version(exe)
}

#[cfg(not(target_os = "macos"))]
fn msix_version(exe: &Path) -> String {
    exe.parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .and_then(|n| n.split('_').nth(1))
        .unwrap_or("?")
        .to_string()
}

#[derive(Clone)]
pub struct Proc {
    pub pid: u32,
    pub exe: String,
    pub cmd: String,
}

#[cfg(test)]
pub fn parse_processes(out: &str) -> Vec<Proc> {
    out.lines()
        .filter_map(|l| {
            let (pid, rest) = l.trim().split_once('|')?;
            let (exe, cmd) = rest.split_once('|')?;
            Some(Proc { pid: pid.parse().ok()?, exe: exe.to_string(), cmd: cmd.to_string() })
        })
        .collect()
}

pub fn processes(app: &App) -> Vec<Proc> {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::processes(app.mac_bundle);
    #[cfg(windows)]
    return snapshot::named(app.exe_name);
    #[cfg(not(windows))]
    vec![]
}

/// The next `processes` call re-reads the process table (after a launch or kill changed it).
fn invalidate() {
    #[cfg(windows)]
    snapshot::invalidate();
}

/// One in-process view of the process table, shared by every caller, so detecting running apps starts no child
/// process. The mutex serialises callers, so checks never overlap; a result under 2 s old is reused.
#[cfg(windows)]
mod snapshot {
    use super::Proc;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use std::time::{Duration, Instant};
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    const MAX_AGE: Duration = Duration::from_secs(2);
    /// Only these are kept with their command line; the rest of the table is used just for parent links.
    const APP_EXES: [&str; 2] = [super::CLAUDE.exe_name, super::CODEX.exe_name];

    struct Snap {
        sys: System,
        at: Option<Instant>,
        /// (lower-case exe name, process)
        procs: Vec<(String, Proc)>,
    }

    static SNAP: LazyLock<Mutex<Snap>> = LazyLock::new(|| Mutex::new(Snap { sys: System::new(), at: None, procs: vec![] }));

    /// Quote an argument that holds spaces the way a Windows command line shows it (`--flag="a b"`).
    fn quoted(a: &str) -> String {
        match a.split_once('=') {
            Some((flag, v)) if a.contains(' ') && flag.starts_with("--") => format!("{flag}=\"{v}\""),
            _ if a.contains(' ') => format!("\"{a}\""),
            _ => a.to_string(),
        }
    }

    fn fresh() -> MutexGuard<'static, Snap> {
        let mut s = SNAP.lock().unwrap_or_else(|e| e.into_inner());
        if s.at.is_some_and(|t| t.elapsed() < MAX_AGE) {
            return s;
        }
        let kind = ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet);
        s.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        let procs = s
            .sys
            .processes()
            .iter()
            .filter_map(|(pid, p)| {
                let name = p.name().to_string_lossy().to_lowercase();
                APP_EXES.iter().any(|n| n.eq_ignore_ascii_case(&name)).then(|| {
                    let proc = Proc {
                        pid: pid.as_u32(),
                        exe: p.exe().map(|e| e.display().to_string()).unwrap_or_default(),
                        cmd: p.cmd().iter().map(|a| quoted(&a.to_string_lossy())).collect::<Vec<_>>().join(" "),
                    };
                    (name, proc)
                })
            })
            .collect();
        s.procs = procs;
        s.at = Some(Instant::now());
        s
    }

    pub fn named(exe_name: &str) -> Vec<Proc> {
        let s = fresh();
        s.procs.iter().filter(|(n, _)| n.eq_ignore_ascii_case(exe_name)).map(|(_, p)| p.clone()).collect()
    }

    pub fn invalidate() {
        SNAP.lock().unwrap_or_else(|e| e.into_inner()).at = None;
    }

    /// This process's parents, nearest first.
    pub fn ancestors() -> Vec<u32> {
        let s = fresh();
        let mut out = vec![];
        let mut pid = Pid::from_u32(std::process::id());
        while let Some(parent) = s.sys.process(pid).and_then(|p| p.parent()) {
            if out.len() > 64 || out.contains(&parent.as_u32()) {
                break;
            }
            out.push(parent.as_u32());
            pid = parent;
        }
        out
    }
}

/// The value of `--user-data-dir=`, quoted or not.
fn user_data_dir(cmd: &str) -> Option<String> {
    let idx = cmd.to_lowercase().find("--user-data-dir=")?;
    let rest = &cmd[idx + 16..];
    Some(match rest.strip_prefix('"') {
        Some(q) => q.split('"').next().unwrap_or("").to_string(),
        // macOS reports the command line unquoted and "Application Support" has a space, so read up to the next flag.
        None if cfg!(target_os = "macos") => rest.split(" --").next().unwrap_or("").trim().to_string(),
        None => rest.split_whitespace().next().unwrap_or("").to_string(),
    })
}

/// pids per profile folder name. Chromium children carry the same flag, so closing kills them too.
pub fn running_from(procs: &[Proc], profiles_dir: &Path) -> HashMap<String, Vec<u32>> {
    let root = profiles_dir.to_string_lossy().to_lowercase();
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    for p in procs {
        let Some(dir) = user_data_dir(&p.cmd) else { continue };
        let dir = PathBuf::from(dir);
        // Case-insensitive exact parent match: nested paths do not count.
        if dir.parent().map(|d| d.to_string_lossy().to_lowercase()) != Some(root.clone()) {
            continue;
        }
        if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
            map.entry(name.to_string()).or_default().push(p.pid);
        }
    }
    map
}

/// Gotcha 1: `claude.exe` is also the Claude Code CLI, so require `<pkg_prefix>*\app\<exe>`.
fn is_packaged_exe(exe: &str, app: &App) -> bool {
    if cfg!(target_os = "macos") {
        return exe.ends_with(&format!("/{0}.app/Contents/MacOS/{0}", app.mac_bundle));
    }
    let p = Path::new(exe);
    let name_ok = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(app.exe_name));
    let parent = p.parent();
    let parent_ok = parent.and_then(Path::file_name).and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("app"));
    let pkg_ok = parent
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(app.package_prefix));
    name_ok && parent_ok && pkg_ok
}

/// Main-instance browser process: packaged exe, no --user-data-dir, no --type=.
pub fn main_pids_from(procs: &[Proc], app: &App) -> Vec<u32> {
    procs
        .iter()
        .filter(|p| {
            let c = p.cmd.to_lowercase();
            is_packaged_exe(&p.exe, app) && !c.contains("--user-data-dir=") && !c.contains("--type=")
        })
        .map(|p| p.pid)
        .collect()
}

/// Every --user-data-dir currently in use by this app (profiles and anything else).
pub fn user_data_dirs(app: &App) -> Vec<String> {
    processes(app).iter().filter_map(|p| user_data_dir(&p.cmd)).map(|d| d.to_lowercase().replace('/', "\\").trim_end_matches('\\').to_string()).collect()
}

pub struct Running {
    pub profiles: HashMap<String, Vec<u32>>,
    pub main: bool,
}

pub fn running(app: &App, profiles_dir: &Path) -> Running {
    let procs = processes(app);
    Running { profiles: running_from(&procs, profiles_dir), main: !main_pids_from(&procs, app).is_empty() }
}

pub fn launch(app: &App, folder: &Path) -> Result<(), String> {
    let exe = find_exe(app).ok_or_else(|| format!("{} not found. Is the desktop app installed?", app.display))?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut envs = vec![];
    for (var, sub) in app.env_overrides {
        let dir = folder.join(sub);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        envs.push((*var, dir));
    }
    if app.key == "codex" {
        crate::terminal::file_store(&folder.join(".codex"))?;
    }
    // `open -n` starts a new instance; plain `open` would just focus the running one.
    #[cfg(target_os = "macos")]
    let mut c = {
        let mut c = Command::new("open");
        c.args(["-n", "-a"]).arg(&exe);
        for (var, dir) in &envs {
            c.arg("--env").arg(format!("{var}={}", dir.display()));
        }
        c.arg("--args").arg(format!("--user-data-dir={}", folder.display()));
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut c = {
        let mut c = Command::new(exe);
        c.arg(format!("--user-data-dir={}", folder.display()));
        c.envs(envs);
        c
    };
    detach(&mut c);
    let r = c.spawn().map(|_| ()).map_err(|e| e.to_string());
    invalidate();
    r
}

pub fn launch_main(app: &App) -> Result<(), String> {
    let exe = find_exe(app).ok_or_else(|| format!("{} not found. Is the desktop app installed?", app.display))?;
    #[cfg(target_os = "macos")]
    let mut c = {
        let mut c = Command::new("open");
        c.arg("-a").arg(&exe);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut c = Command::new(exe);
    detach(&mut c);
    let r = c.spawn().map(|_| ()).map_err(|e| e.to_string());
    invalidate();
    r
}

fn detach(c: &mut Command) {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0000_0208); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }
}

/// Tandem's own parent chain. If Tandem was started from a Claude process, killing that tree would kill Tandem too.
fn my_ancestors() -> Vec<u32> {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::ancestors();
    #[cfg(windows)]
    return snapshot::ancestors();
    #[cfg(not(windows))]
    vec![]
}

/// Force kill with the process tree; there is no graceful close. Tandem's own ancestors are killed without /T.
pub fn kill_pids(pids: &[u32]) -> usize {
    let mine = my_ancestors();
    #[cfg(target_os = "macos")]
    return pids.iter().filter(|pid| Command::new("kill").args(["-9", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())).count();
    let killed = pids
        .iter()
        .filter(|pid| {
            let mut c = Command::new("taskkill");
            c.args(["/PID", &pid.to_string()]);
            if !mine.contains(pid) {
                c.arg("/T");
            }
            c.arg("/F").stdout(Stdio::null()).stderr(Stdio::null());
            hide(&mut c);
            c.status().map(|s| s.success()).unwrap_or(false)
        })
        .count();
    invalidate();
    killed
}

/// Retry every 0.5 s for up to 15 s: file locks linger after a kill. Clears the read-only bit (Git pack files).
pub fn remove_dir_retry(path: &Path) -> Result<(), String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if !path.exists() {
            return Ok(());
        }
        clear_readonly(path);
        if std::fs::remove_dir_all(path).is_ok() && !path.exists() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("Could not remove {}: files still in use.", path.display()));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn clear_readonly(path: &Path) {
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let p = e.path();
            if let Ok(m) = p.metadata() {
                let mut perm = m.permissions();
                if perm.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    let _ = std::fs::set_permissions(&p, perm);
                }
                if m.is_dir() {
                    clear_readonly(&p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the live process table (read-only): every running Claude process must come back with its exe, and a
    /// `--user-data-dir` must parse to the same folder as the real command line.
    #[cfg(windows)]
    #[test]
    fn in_process_listing_matches_wmi() {
        let ours = processes(&CLAUDE);
        let wmi = parse_processes(&powershell("Get-CimInstance Win32_Process -Filter \"Name='claude.exe'\" | ForEach-Object { \"$($_.ProcessId)|$($_.ExecutablePath)|$($_.CommandLine)\" }"));
        for w in wmi.iter().filter(|w| !w.cmd.is_empty()) {
            // A process may have exited between the two reads.
            let Some(o) = ours.iter().find(|o| o.pid == w.pid) else { continue };
            // Names only: inside the MSIX-virtualised Claude app the two readers can show different %APPDATA% roots.
            let name = |e: &str| Path::new(e).file_name().map(|n| n.to_string_lossy().to_lowercase());
            assert_eq!(name(&o.exe), name(&w.exe), "exe of {}", w.pid);
            assert_eq!(user_data_dir(&o.cmd).map(|d| d.to_lowercase()), user_data_dir(&w.cmd).map(|d| d.to_lowercase()), "user-data-dir of {}", w.pid);
            assert_eq!(is_packaged_exe(&o.exe, &CLAUDE), is_packaged_exe(&w.exe, &CLAUDE));
        }
        assert!(!ours.is_empty() || wmi.is_empty());
        assert!(!my_ancestors().is_empty());
    }

    const PKG: &str = r"C:\Program Files\WindowsApps\Claude_3.6.2.0_x64__pzs8sxrjxfjjc\app\Claude.exe";

    fn p(pid: u32, exe: &str, cmd: &str) -> Proc {
        Proc { pid, exe: exe.into(), cmd: cmd.into() }
    }

    #[test]
    fn parses_process_lines_and_skips_junk() {
        let v = parse_processes("12|C:\\a.exe|a.exe --x\nbad line\nx|y|z\n34|C:\\b.exe|b.exe\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].pid, 34);
    }

    #[cfg(windows)]
    #[test]
    fn running_matches_exact_parent_case_insensitively() {
        let root = Path::new(r"D:\Tandem\ClaudeProfiles");
        let procs = [
            p(1, PKG, r#"Claude.exe --user-data-dir="d:\tandem\claudeprofiles\Work""#),
            p(2, PKG, r#"Claude.exe --type=renderer --user-data-dir=D:\Tandem\ClaudeProfiles\Work"#),
            p(3, PKG, r"Claude.exe --user-data-dir=D:\Tandem\ClaudeProfiles\Work\nested"),
            p(4, PKG, r"Claude.exe --user-data-dir=D:\Other\Work"),
            p(5, PKG, "Claude.exe"),
        ];
        let r = running_from(&procs, root);
        assert_eq!(r.len(), 1);
        assert_eq!(r["Work"], vec![1, 2]);
    }

    #[cfg(windows)]
    #[test]
    fn main_instance_ignores_cli_children_and_profiles() {
        let procs = [
            p(1, PKG, "Claude.exe"),
            p(2, PKG, "Claude.exe --type=gpu-process"),
            p(3, PKG, r"Claude.exe --user-data-dir=D:\x\y"),
            p(4, r"C:\Users\me\.local\bin\claude.exe", "claude.exe"),
        ];
        assert_eq!(main_pids_from(&procs, &CLAUDE), vec![1]);
    }

    #[cfg(windows)]
    #[test]
    fn version_from_install_path() {
        assert_eq!(app_version(Path::new(PKG)), "3.6.2.0");
        assert_eq!(app_version(Path::new("x.exe")), "?");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_command_lines_with_spaces() {
        let cmd = "/Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/me/Library/Application Support/Tandem/ClaudeProfiles/Work --flag";
        assert_eq!(user_data_dir(cmd).unwrap(), "/Users/me/Library/Application Support/Tandem/ClaudeProfiles/Work");
        assert!(is_packaged_exe("/Applications/Claude.app/Contents/MacOS/Claude", &CLAUDE));
        assert!(!is_packaged_exe("/Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper", &CLAUDE));
    }

    /// Manual: launches Claude on a throwaway profile folder, closes it the way the Close button does, and reports what is left.
    /// `TANDEM_SCRATCH=<empty folder> cargo test close_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn close_live() {
        let root = std::path::PathBuf::from(std::env::var("TANDEM_SCRATCH").unwrap());
        let folder = root.join("CloseTest");
        launch(&CLAUDE, &folder).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(8));
        let pids = running(&CLAUDE, &root).profiles.into_iter().find(|(k, _)| k == "CloseTest").map(|(_, v)| v).unwrap_or_default();
        println!("found {} processes: {:?}", pids.len(), pids);
        println!("killed {}", kill_pids(&pids));
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!("left: {:?}", running(&CLAUDE, &root).profiles.get("CloseTest"));
    }
}

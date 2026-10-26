//! macOS: find the desktop apps, list their processes, read the Keychain, open terminals.
use crate::engine::Proc;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// `/Applications/<name>.app`, then `~/Applications`.
pub fn find_bundle(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    [Some(PathBuf::from("/Applications")), home.map(|h| h.join("Applications"))]
        .into_iter()
        .flatten()
        .map(|d| d.join(format!("{name}.app")))
        .find(|p| p.is_dir())
}

pub fn bundle_version(bundle: &Path) -> String {
    let v = run("defaults", &["read", &bundle.join("Contents/Info").to_string_lossy(), "CFBundleShortVersionString"]);
    let v = v.trim();
    if v.is_empty() { "?".into() } else { v.into() }
}

/// Processes whose command line runs from `<bundle>.app` (helpers included). `exe` is the binary's path.
pub fn processes(bundle: &str) -> Vec<Proc> {
    let marker = ".app/Contents/MacOS/";
    run("ps", &["-axww", "-o", "pid=,command="])
        .lines()
        .filter_map(|l| {
            let (pid, cmd) = l.trim().split_once(' ')?;
            let cmd = cmd.trim_start();
            let i = cmd.find(marker)?;
            if !cmd.starts_with('/') || !cmd[..i].contains(&format!("/{bundle}")) {
                return None;
            }
            let after = &cmd[i + marker.len()..];
            let bin = after.split(" --").next().unwrap_or(after);
            Some(Proc { pid: pid.parse().ok()?, exe: cmd[..i + marker.len() + bin.len()].to_string(), cmd: cmd.to_string() })
        })
        .collect()
}

/// Tandem's own parent chain.
pub fn ancestors() -> Vec<u32> {
    let mut out = vec![];
    let mut p = std::process::id();
    while let Ok(n) = run("ps", &["-o", "ppid=", "-p", &p.to_string()]).trim().parse::<u32>() {
        if n <= 1 || out.contains(&n) {
            break;
        }
        out.push(n);
        p = n;
    }
    out
}

/// The password behind "<Product> Safe Storage". macOS asks the user to allow this the first time.
pub fn safe_storage_password(product: &str) -> Option<String> {
    let out = run("security", &["find-generic-password", "-w", "-s", &format!("{product} Safe Storage"), "-a", product]);
    let p = out.trim_end_matches('\n');
    (!p.is_empty()).then(|| p.to_string())
}

pub fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Runs `script_body` in a new window of Terminal, iTerm or Ghostty via a throwaway .command file.
pub fn open_terminal(which: &str, title: &str, script_body: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let file = std::env::temp_dir().join(format!("tandem-{}-{nanos}.command", std::process::id()));
    let text = format!("#!/bin/zsh -l\nrm -f -- \"$0\"\nprintf '\033]0;%s\007' {}\n{script_body}\nexec \"$SHELL\" -l\n", sq(title));
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let mut c = Command::new("open");
    match which {
        "ghostty" => c.args(["-na", "Ghostty", "--args", "-e"]).arg(&file),
        "iterm" => c.args(["-a", "iTerm"]).arg(&file),
        _ => c.args(["-a", "Terminal"]).arg(&file),
    };
    let ok = c.stdin(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(()) } else { Err(format!("Couldn't open {}. Is it installed?", match which { "ghostty" => "Ghostty", "iterm" => "iTerm", _ => "Terminal" })) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_quotes_survive_the_shell() {
        assert_eq!(sq("it's"), r"'it'\''s'");
    }
}

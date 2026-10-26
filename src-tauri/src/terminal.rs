//! Terminal profiles: Claude Code and Codex CLI, each run with its own config folder.
//! Claude Code reads `CLAUDE_CONFIG_DIR`; Codex CLI reads `CODEX_HOME`. Both point at the profile's folder.
use crate::engine::App;
use crate::store::Settings;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

pub struct Tool {
    pub version: String,
    pub path: String,
}

fn spec(app: &App) -> (&'static str, &'static str) {
    // (command, env var)
    if app.key == "cc" { ("claude", "CLAUDE_CONFIG_DIR") } else { ("codex", "CODEX_HOME") }
}

#[cfg(windows)]
fn hidden(c: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000)
}
#[cfg(not(windows))]
fn hidden(c: &mut Command) -> &mut Command {
    c
}

/// Version and location of the CLI on PATH, cached for a minute (`npm` shims need `cmd`, so this is slow).
pub fn detect(app: &App) -> Option<Tool> {
    static CACHE: Mutex<Vec<(&'static str, Instant, Option<(String, String)>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, at, v)) = cache.iter().find(|(k, _, _)| *k == app.key) {
        if at.elapsed().as_secs() < 60 {
            return v.clone().map(|(version, path)| Tool { version, path });
        }
    }
    let (cmd, _) = spec(app);
    let found = probe(cmd);
    cache.retain(|(k, _, _)| *k != app.key);
    cache.push((app.key, Instant::now(), found.clone()));
    found.map(|(version, path)| Tool { version, path })
}

#[cfg(target_os = "macos")]
fn probe(cmd: &str) -> Option<(String, String)> {
    // A login shell, because apps started from the Dock don't inherit the PATH of the user's terminal.
    let sh = |script: String| Command::new("/bin/zsh").args(["-lc", &script]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let path = sh(format!("command -v {cmd}"))?.lines().last()?.trim().to_string();
    let text = sh(format!("{cmd} --version"))?;
    let version = text.split_whitespace().find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?.to_string();
    Some((version, path))
}

#[cfg(not(target_os = "macos"))]
fn probe(cmd: &str) -> Option<(String, String)> {
    if !cfg!(windows) {
        return None;
    }
    let out = hidden(Command::new("cmd").args(["/c", cmd, "--version"]).stdin(Stdio::null())).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let version = text.split_whitespace().find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?.to_string();
    let w = hidden(Command::new("where").arg(cmd).stdin(Stdio::null())).output().ok()?;
    let path = String::from_utf8_lossy(&w.stdout).lines().next().unwrap_or("").trim().to_string();
    Some((version, path))
}

fn creds_file(app: &App, folder: &Path) -> PathBuf {
    folder.join(if app.key == "cc" { ".credentials.json" } else { "auth.json" })
}

pub fn signed_in(app: &App, folder: &Path) -> bool {
    creds_file(app, folder).is_file()
}

/// The account's email, read from the files the tool itself writes. Nothing secret is returned.
pub fn account(app: &App, folder: &Path) -> String {
    let read = |p: PathBuf| std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or_default();
    if app.key == "cc" {
        return read(folder.join(".claude.json"))["oauthAccount"]["emailAddress"].as_str().unwrap_or("").to_string();
    }
    // Codex keeps the email inside the id token's payload (the middle part of the JWT).
    let tok = read(folder.join("auth.json"))["tokens"]["id_token"].as_str().unwrap_or("").to_string();
    let payload = tok.split('.').nth(1).and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok());
    payload.and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v["email"].as_str().map(String::from)).unwrap_or_default()
}

/// Codex must keep its sign-in in a file inside the profile, not the system credential store.
pub fn prepare(app: &App, folder: &Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    if app.key == "cx" {
        file_store(folder)?;
    }
    Ok(())
}

/// Makes Codex keep its sign-in in `auth.json` inside `dir` instead of the Keychain or Credential Manager.
pub fn file_store(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let cfg = dir.join("config.toml");
    let old = std::fs::read_to_string(&cfg).unwrap_or_default();
    if !old.contains("cli_auth_credentials_store") {
        std::fs::write(&cfg, format!("cli_auth_credentials_store = \"file\"\n{old}")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn q(s: &str) -> String {
    s.replace('\'', "''")
}

/// Opens a terminal for the profile. `login` runs the sign-in command instead of the plain tool.
pub fn open(app: &App, name: &str, folder: &Path, s: &Settings, login: bool) -> Result<(), String> {
    let run = if login && app.key == "cx" { "codex login" } else { spec(app).0 };
    start(app, name, folder, s, run, None)
}

/// Sends one message through the Claude Code CLI (`claude -p`) signed in as the profile in `folder`, and waits for it to finish.
/// API-key variables are removed so the plan's login is used, never a paid API key. Errors are fixed text.
pub fn run_prompt(folder: &Path, message: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut c = {
        use crate::platform::macos::sq;
        let script = format!("export CLAUDE_CONFIG_DIR={}; unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN; claude -p {}", sq(&folder.to_string_lossy()), sq(message));
        let mut c = Command::new("/bin/zsh");
        c.args(["-lc", &script]);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut c = {
        let script = format!(
            "$env:CLAUDE_CONFIG_DIR='{}'; foreach($v in 'ANTHROPIC_API_KEY','ANTHROPIC_AUTH_TOKEN'){{ Remove-Item \"Env:$v\" -ErrorAction SilentlyContinue }}; claude -p '{}'; exit $LASTEXITCODE",
            q(&folder.to_string_lossy()),
            q(message)
        );
        let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut c = Command::new("powershell.exe");
        c.args(["-NoLogo", "-NonInteractive", "-EncodedCommand", &B64.encode(utf16)]);
        c
    };
    hidden(&mut c);
    let mut child = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_| "Couldn't run Claude Code.".to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) if s.success() => return Ok(()),
            Ok(Some(_)) => return Err("Claude Code didn't send it. Check that the profile is signed in and you're online.".into()),
            Ok(None) if start.elapsed().as_secs() < 120 => std::thread::sleep(std::time::Duration::from_millis(500)),
            _ => {
                let _ = child.kill();
                return Err("Claude Code took too long to answer.".into());
            }
        }
    }
}

/// Resumes a past session in its own folder. `sid` must already be checked to be letters, digits and dashes.
pub fn resume(app: &App, root: &Path, s: &Settings, sid: &str, cwd: Option<&str>) -> Result<(), String> {
    let run = if app.key == "cc" { format!("claude --resume {sid}") } else { format!("codex resume {sid}") };
    start(app, "Resume", root, s, &run, cwd)
}

fn start(app: &App, name: &str, folder: &Path, s: &Settings, run: &str, cwd: Option<&str>) -> Result<(), String> {
    prepare(app, folder)?;
    let (_, var) = spec(app);
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::{open_terminal, sq};
        let clears = if s.keep_separate { "unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY\n" } else { "" };
        let go = cwd.map(|d| format!("cd {} 2>/dev/null\n", sq(d))).unwrap_or_default();
        return open_terminal(&s.terminal, name, &format!("export {var}={}\n{clears}{go}{run}", sq(&folder.to_string_lossy())));
    }
    #[cfg(not(target_os = "macos"))]
    start_windows(name, folder, s, run, cwd, var)
}

#[cfg(not(target_os = "macos"))]
fn start_windows(name: &str, folder: &Path, s: &Settings, run: &str, cwd: Option<&str>, var: &str) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Terminal profiles need Windows or macOS.".into());
    }
    let go = cwd.map(|d| format!("Set-Location -LiteralPath '{}' -ErrorAction SilentlyContinue; ", q(d))).unwrap_or_default();
    let clears = if s.keep_separate { "foreach($v in 'ANTHROPIC_API_KEY','ANTHROPIC_AUTH_TOKEN','OPENAI_API_KEY'){ Remove-Item \"Env:$v\" -ErrorAction SilentlyContinue }; " } else { "" };
    let script = format!("$Host.UI.RawUI.WindowTitle='{}'; $env:{var}='{}'; {clears}{go}{run}", q(name), q(&folder.to_string_lossy()));
    // Encoded so quotes and the semicolons Windows Terminal treats as separators can't break the command.
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let enc = B64.encode(utf16);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let wt = s.terminal != "powershell" && Command::new("wt").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x0800_0000).status().is_ok();
        let mut c = if wt {
            let mut c = Command::new("wt");
            c.args(["-w", "-1", "new-tab", "powershell.exe", "-NoLogo", "-NoExit", "-EncodedCommand", &enc]);
            c.creation_flags(0x0800_0000);
            c
        } else {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoLogo", "-NoExit", "-EncodedCommand", &enc]);
            c.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE
            c
        };
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        c.spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = enc;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{CLAUDE_CODE, CODEX_CLI};

    #[test]
    fn codex_config_gets_file_store_once() {
        let d = std::env::temp_dir().join(format!("tandem-term-test-{}", std::process::id()));
        prepare(&CODEX_CLI, &d).unwrap();
        prepare(&CODEX_CLI, &d).unwrap();
        let t = std::fs::read_to_string(d.join("config.toml")).unwrap();
        assert_eq!(t.matches("cli_auth_credentials_store").count(), 1);
        prepare(&CLAUDE_CODE, &d.join("cc")).unwrap();
        assert!(!d.join("cc").join("config.toml").exists());
        let _ = std::fs::remove_dir_all(d);
    }
}

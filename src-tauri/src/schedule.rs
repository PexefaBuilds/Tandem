//! Starts a Claude 5-hour window by sending one short message: through the Claude Code CLI (preferred), or BETA through the web route.
//! BETA web route: start a Claude 5-hour window at a chosen time by sending one short message in a new chat.
//! Uses claude.ai's unofficial web API with the profile's saved login, so it can break or be blocked at any time.
//! It only runs while Tandem is open.
use crate::{engine, store, terminal, usage};
use chrono::{Local, NaiveTime, TimeZone};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// A start more than this long after its time is "missed", not sent: the window would begin much later than planned.
const GRACE_SECS: i64 = 10 * 60;

pub fn valid_time(t: &str) -> bool {
    NaiveTime::parse_from_str(t, "%H:%M").is_ok()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A random version-4 UUID without a new dependency: hash the clock, process id and a counter.
fn uuid_v4() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let mut h = Sha256::new();
    h.update(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0).to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(N.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let x: String = b.iter().map(|v| format!("{v:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &x[0..8], &x[8..12], &x[12..16], &x[16..20], &x[20..32])
}

/// Sends the message. Errors are fixed, user-facing text (never a cookie or header).
pub async fn send(app: &str, name: &str, message: &str, route: &str, terminal: &str) -> Result<(), String> {
    if route == "terminal" {
        return send_terminal(app, message, terminal).await;
    }
    if app != "claude" {
        return Err("Only Claude profiles can start a session.".into());
    }
    let a = engine::app_by_key(app).ok_or("Unknown app")?;
    let folder = store::load_meta().profiles.into_iter().find(|m| m.app == app && m.name == name).map(|m| m.folder).ok_or("Profile not found")?;
    let path = store::data_dir().join(a.profiles_dirname).join(&folder);
    let c = usage::client().ok_or("Couldn't start the network client")?;
    let (cookie, org) = usage::claude_login(&c, app, &folder, &path).await?;
    let post = |url: String, body: serde_json::Value| {
        c.post(url).header("Cookie", &cookie).header("Referer", "https://claude.ai/").header("Origin", "https://claude.ai").header("Accept", "*/*").json(&body)
    };
    let id = uuid_v4();
    let base = format!("https://claude.ai/api/organizations/{org}/chat_conversations");
    let r = post(base.clone(), json!({ "uuid": id, "name": "" })).send().await.map_err(|_| "Network unavailable".to_string())?;
    if !r.status().is_success() {
        return Err(usage::http_fail(r.status()));
    }
    let r = post(format!("{base}/{id}/completion"), json!({ "prompt": message, "attachments": [], "files": [] })).send().await.map_err(|_| "Network unavailable".to_string())?;
    if !r.status().is_success() {
        return Err(usage::http_fail(r.status()));
    }
    // The reply streams in; reading it to the end lets the chat finish cleanly. The message is already counted.
    let _ = r.bytes().await;
    Ok(())
}

/// The preferred route: `claude -p` through a signed-in Claude Code profile.
async fn send_terminal(app: &str, message: &str, terminal: &str) -> Result<(), String> {
    if app != "claude" {
        return Err("Only Claude profiles can start a session.".into());
    }
    let cc = engine::app_by_key("cc").ok_or("Unknown app")?;
    let folder = store::load_meta().profiles.into_iter().find(|m| m.app == "cc" && m.name == terminal).map(|m| m.folder).ok_or("Pick a Claude Code profile.")?;
    let path = store::data_dir().join(cc.profiles_dirname).join(folder);
    if !terminal::signed_in(&cc, &path) {
        return Err("That Claude Code profile isn't signed in.".into());
    }
    let message = message.to_string();
    tauri::async_runtime::spawn_blocking(move || terminal::run_prompt(&path, &message)).await.map_err(|_| "Couldn't run Claude Code.".to_string())?
}

/// Sends now and records the outcome on the profile's schedule (creating no schedule if there is none).
pub async fn send_and_record(app: &str, name: &str, message: &str, route: &str, terminal: &str) -> Result<(), String> {
    let res = send(app, name, message, route, terminal).await;
    let _g = store::lock();
    let mut f = store::load_schedules();
    if let Some(s) = f.schedules.iter_mut().find(|s| s.app == app && s.name == name) {
        match &res {
            Ok(()) => {
                s.last_sent = now_secs();
                s.last_status = "sent".into();
                s.last_note.clear();
            }
            Err(e) => {
                s.last_status = "failed".into();
                s.last_note = e.clone();
            }
        }
        let _ = store::save_schedules(&f);
    }
    res
}

fn notify(h: &AppHandle, title: String, body: String) {
    let _ = h.notification().builder().title(title).body(body).show();
}

async fn tick(h: &AppHandle) {
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let due: Vec<store::Schedule> = store::load_schedules().schedules.into_iter().filter(|s| s.enabled && s.last_day != today).collect();
    for s in due {
        let Ok(t) = NaiveTime::parse_from_str(&s.time, "%H:%M") else { continue };
        let Some(target) = Local.from_local_datetime(&now.date_naive().and_time(t)).earliest() else { continue };
        let late = (now - target).num_seconds();
        if late < 0 {
            continue;
        }
        // Mark the day first so a slow send can't fire twice.
        {
            let _g = store::lock();
            let mut f = store::load_schedules();
            if let Some(x) = f.schedules.iter_mut().find(|x| x.app == s.app && x.name == s.name) {
                x.last_day = today.clone();
                if s.repeat == "once" {
                    x.enabled = false;
                }
                if late > GRACE_SECS {
                    x.last_status = "missed".into();
                    x.last_note = format!("Tandem wasn't running at {}", s.time);
                }
                let _ = store::save_schedules(&f);
            }
        }
        if late > GRACE_SECS {
            continue;
        }
        match send_and_record(&s.app, &s.name, &s.message, &s.route, &s.terminal).await {
            Ok(()) => notify(h, format!("Session started: {}", s.name), "Your 5-hour window has begun.".into()),
            Err(e) => notify(h, format!("Couldn't start a session for {}", s.name), e),
        }
    }
}

pub fn start(h: AppHandle) {
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(tick(&h));
        std::thread::sleep(Duration::from_secs(20));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_uuids() {
        assert!(valid_time("08:05") && valid_time("23:59"));
        assert!(!valid_time("8") && !valid_time("24:00") && !valid_time(""));
        let (a, b) = (uuid_v4(), uuid_v4());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
    }
}

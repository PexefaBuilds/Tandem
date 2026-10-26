//! Usage limits per profile. Claude: the claude.ai web session read from the profile's cookie store.
//! Codex: the OAuth token in the profile's `.codex/auth.json`. Secrets stay in memory (or DPAPI-protected on disk).
use crate::{engine, signin, store};
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";
#[cfg(not(target_os = "macos"))]
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub label: String,
    pub pct: f64,
    pub resets_at: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub app: String,
    pub name: String,
    /// "" when the last refresh worked; otherwise "expired" | "locked" | "offline" | "limited" | "blocked" | "other".
    pub kind: String,
    pub message: String,
    pub account: String,
    pub plan: String,
    pub rows: Vec<Row>,
    /// Unix seconds of the last successful refresh; 0 = never.
    pub updated: u64,
}

enum Fail {
    Expired,
    Locked,
    NotSignedIn,
    Offline,
    Blocked,
    Limited,
    Other(String),
}

impl Fail {
    fn parts(&self) -> (&'static str, String) {
        match self {
            Fail::Expired => ("expired", "Session expired".into()),
            Fail::Locked => ("locked", "Open it once to refresh".into()),
            Fail::NotSignedIn => ("expired", "Not signed in".into()),
            Fail::Offline => ("offline", "Network unavailable".into()),
            Fail::Blocked => ("blocked", "Blocked by Cloudflare".into()),
            Fail::Limited => ("limited", "Rate-limited, trying again later".into()),
            Fail::Other(m) => ("other", m.clone()),
        }
    }
}

struct Got {
    account: String,
    plan: String,
    rows: Vec<Row>,
}

#[derive(Default)]
struct Mem {
    last: HashMap<String, Usage>,
    /// key -> (strikes, retry-after unix secs)
    limited: HashMap<String, (u32, u64)>,
    notified: HashSet<String>,
}

static MEM: LazyLock<Mutex<Mem>> = LazyLock::new(Default::default);
static CACHE_LOCK: Mutex<()> = Mutex::new(());

fn mem() -> MutexGuard<'static, Mem> {
    MEM.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn key(app: &str, folder: &str) -> String {
    format!("{app}:{}", folder.to_lowercase())
}

// ---- DPAPI + Chromium cookies -------------------------------------------------------------------------------

#[cfg(windows)]
fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if protect {
            CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut out).ok()?;
        } else {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut out).ok()?;
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out.pbData as *mut _));
        Some(v)
    }
}

#[cfg(not(windows))]
fn dpapi(_: &[u8], _: bool) -> Option<Vec<u8>> {
    None // the on-disk session cache is Windows-only; on macOS the cookie database can be read while the app runs
}

/// macOS Chromium: AES-128 key from the Keychain password (PBKDF2-SHA1, salt "saltysalt", 1003 rounds).
fn mac_key(password: &str) -> Vec<u8> {
    let mut k = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), b"saltysalt", 1003, &mut k);
    k.to_vec()
}

#[cfg(target_os = "macos")]
fn master_key(_: &Path) -> Option<Vec<u8>> {
    static KEY: Mutex<Option<Vec<u8>>> = Mutex::new(None);
    let mut k = KEY.lock().unwrap_or_else(|e| e.into_inner());
    if k.is_none() {
        *k = crate::platform::macos::safe_storage_password("Claude").map(|p| mac_key(&p));
    }
    k.clone()
}

#[cfg(not(target_os = "macos"))]
fn master_key(local_state: &Path) -> Option<Vec<u8>> {
    let j: Value = serde_json::from_slice(&std::fs::read(local_state).ok()?).ok()?;
    let raw = B64.decode(j["os_crypt"]["encrypted_key"].as_str()?).ok()?;
    dpapi(raw.strip_prefix(b"DPAPI")?, false)
}

fn decrypt_cookie(key: &[u8], host: &str, blob: &[u8]) -> Option<String> {
    if blob.len() > 31 && (blob.starts_with(b"v10") || blob.starts_with(b"v11")) {
        let mut raw = if key.len() == 16 {
            cbc::Decryptor::<aes::Aes128>::new_from_slices(key, &[b' '; 16]).ok()?.decrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&blob[3..]).ok()?
        } else {
            Aes256Gcm::new_from_slice(key).ok()?.decrypt(Nonce::from_slice(&blob[3..15]), &blob[15..]).ok()?
        };
        // Recent Chromium prefixes the value with SHA256(host_key).
        if raw.starts_with(&Sha256::digest(host.as_bytes())) {
            raw.drain(..32);
        }
        String::from_utf8(raw).ok()
    } else {
        String::from_utf8(dpapi(blob, false)?).ok()
    }
}

enum Session {
    Live(String, Option<String>),
    Absent,
    Locked,
}

fn sqlite_uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/").replace('%', "%25").replace(' ', "%20").replace('#', "%23").replace('?', "%3F");
    format!("file:///{}?mode=ro&immutable=1", s.trim_start_matches('/'))
}

/// A running profile holds its cookie database open exclusively, so this reports `Locked` then.
fn read_session(folder: &Path) -> Session {
    let (db, ls) = (folder.join("Network").join("Cookies"), folder.join("Local State"));
    if !db.is_file() || !ls.is_file() {
        return Session::Absent;
    }
    let Some(mk) = master_key(&ls) else { return Session::Locked };
    let read = || -> rusqlite::Result<HashMap<String, String>> {
        let conn = rusqlite::Connection::open_with_flags(sqlite_uri(&db), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI)?;
        let mut st = conn.prepare("SELECT host_key, name, encrypted_value FROM cookies WHERE host_key LIKE '%claude.ai%'")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Vec<u8>>(2)?)))?;
        let mut jar = HashMap::new();
        for (host, name, blob) in rows.flatten() {
            if let Some(v) = (!blob.is_empty()).then(|| decrypt_cookie(&mk, &host, &blob)).flatten() {
                jar.insert(name, v);
            }
        }
        Ok(jar)
    };
    match read() {
        Ok(mut jar) => match jar.remove("sessionKey") {
            Some(s) => Session::Live(s, jar.remove("cf_clearance")),
            None => Session::Absent,
        },
        Err(_) => Session::Locked,
    }
}

// ---- Encrypted session cache (for profiles that are open) ---------------------------------------------------

fn cache_file() -> std::path::PathBuf {
    store::data_dir().join("usage_cache.json")
}

fn cache_load() -> HashMap<String, String> {
    std::fs::read_to_string(cache_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn cache_save(m: &HashMap<String, String>) {
    let tmp = cache_file().with_extension("json.tmp");
    let _ = std::fs::create_dir_all(store::data_dir());
    if serde_json::to_string_pretty(m).ok().is_some_and(|s| std::fs::write(&tmp, s).is_ok()) {
        let _ = std::fs::rename(&tmp, cache_file());
    }
}

fn cache_put(k: &str, session: &str) {
    let _g = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(enc) = dpapi(session.as_bytes(), true) {
        let mut m = cache_load();
        m.insert(k.into(), B64.encode(enc));
        cache_save(&m);
    }
}

fn cache_get(k: &str) -> Option<String> {
    let _g = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    String::from_utf8(dpapi(&B64.decode(cache_load().get(k)?).ok()?, false)?).ok()
}

fn cache_drop(k: &str) {
    let _g = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = cache_load();
    if m.remove(k).is_some() {
        cache_save(&m);
    }
}

/// Forget a profile's cached session (the profile was deleted or signed in again).
pub fn forget(app: &str, folder: &str) {
    cache_drop(&key(app, folder));
}

// ---- Fetching -----------------------------------------------------------------------------------------------

fn status_fail(s: reqwest::StatusCode, expired: Fail) -> Fail {
    match s.as_u16() {
        401 => expired,
        403 => Fail::Blocked,
        429 => Fail::Limited,
        c => Fail::Other(format!("HTTP {c}")),
    }
}

async fn get_json(c: &reqwest::Client, url: &str, req: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder, expired: Fail) -> Result<Value, Fail> {
    // Errors are mapped to fixed text so no header or cookie can leak into a message.
    let resp = req(c.get(url).header("Accept", "*/*")).send().await.map_err(|_| Fail::Offline)?;
    if !resp.status().is_success() {
        return Err(status_fail(resp.status(), expired));
    }
    resp.json().await.map_err(|_| Fail::Other("Unexpected reply".into()))
}

fn pick_org(orgs: &[Value]) -> Option<&Value> {
    let has_chat = |o: &&Value| o["capabilities"].as_array().is_some_and(|c| c.iter().any(|x| x == "chat"));
    let chat: Vec<&Value> = orgs.iter().filter(has_chat).collect();
    let pool: Vec<&Value> = if chat.is_empty() { orgs.iter().collect() } else { chat };
    pool.iter().find(|o| o["raven_type"] == "team").or(pool.first()).copied()
}

fn account_label(name: &str) -> String {
    let n = name.trim_end_matches("'s Organization").trim_end_matches("’s Organization").trim();
    if n.is_empty() { "—".into() } else { n.into() }
}

fn has_cap(org: &Value, cap: &str) -> bool {
    org["capabilities"].as_array().is_some_and(|c| c.iter().any(|x| x == cap))
}

fn plan_of(org: &Value) -> String {
    if org["raven_type"] == "team" {
        return "Team".into();
    }
    let tier = org["rate_limit_tier"].as_str().unwrap_or("").to_lowercase();
    match () {
        _ if tier.contains("max_20x") => "Max 20x",
        _ if tier.contains("max_5x") => "Max 5x",
        _ if tier.contains("max") => "Max",
        _ if tier.contains("pro") => "Pro",
        // Pro's tier is a generic "default_claude_ai", so fall back to the organization's capabilities.
        _ if has_cap(org, "claude_max") => "Max",
        _ if has_cap(org, "claude_pro") => "Pro",
        _ => "",
    }
    .into()
}

fn claude_rows(d: &Value) -> Vec<Row> {
    let mut rows = vec![];
    for (label, k) in [("5h", "five_hour"), ("7d", "seven_day")] {
        if let Some(pct) = d[k]["utilization"].as_f64() {
            rows.push(Row { label: label.into(), pct, resets_at: d[k]["resets_at"].as_str().unwrap_or("").into() });
        }
    }
    for lim in d["limits"].as_array().into_iter().flatten().filter(|l| l["kind"] == "weekly_scoped") {
        if let (Some(model), Some(pct)) = (lim["scope"]["model"]["display_name"].as_str(), lim["percent"].as_f64()) {
            rows.push(Row { label: model.into(), pct, resets_at: lim["resets_at"].as_str().unwrap_or("").into() });
        }
    }
    rows
}

/// The Cookie header for a Claude profile: read from its cookie file, or the cached copy while the profile is open.
async fn claude_cookie(k: &str, folder: &Path) -> Result<String, Fail> {
    let f = folder.to_path_buf();
    let session = tauri::async_runtime::spawn_blocking(move || read_session(&f)).await.map_err(|_| Fail::Locked)?;
    let (sk, cf) = match session {
        Session::Live(s, cf) => {
            // Cached as "sessionKey\ncf_clearance": Cloudflare blocks the key alone while the profile is open.
            cache_put(k, &format!("{s}\n{}", cf.as_deref().unwrap_or("")));
            (s, cf)
        }
        other => {
            let c = cache_get(k).ok_or(if matches!(other, Session::Locked) { Fail::Locked } else { Fail::NotSignedIn })?;
            let (s, cf) = c.split_once('\n').unwrap_or((&c, ""));
            (s.to_string(), (!cf.is_empty()).then(|| cf.to_string()))
        }
    };
    let cookie = match cf {
        Some(cf) => format!("sessionKey={sk}; cf_clearance={cf}"),
        None => format!("sessionKey={sk}"),
    };
    Ok(cookie)
}

async fn fetch_claude(c: &reqwest::Client, k: &str, folder: &Path) -> Result<Got, Fail> {
    let cookie = claude_cookie(k, folder).await?;
    let req = |r: reqwest::RequestBuilder| r.header("Cookie", cookie.clone()).header("Referer", "https://claude.ai/");
    let orgs = get_json(c, "https://claude.ai/api/organizations", req, Fail::Expired).await;
    if matches!(orgs, Err(Fail::Expired)) {
        cache_drop(k);
    }
    let orgs = orgs?;
    let org = pick_org(orgs.as_array().map(Vec::as_slice).unwrap_or(&[])).ok_or_else(|| Fail::Other("No organization".into()))?;
    let id = org["uuid"].as_str().or(org["id"].as_str()).ok_or_else(|| Fail::Other("No organization".into()))?;
    let data = get_json(c, &format!("https://claude.ai/api/organizations/{id}/usage"), req, Fail::Expired).await?;
    Ok(Got { account: account_label(org["name"].as_str().unwrap_or("")), plan: plan_of(org), rows: claude_rows(&data) })
}

fn window_label(secs: Option<i64>) -> String {
    match secs {
        Some(s) if s >= 86400 => format!("{}d", s / 86400),
        Some(s) => format!("{}h", s / 3600),
        None => "?".into(),
    }
}

async fn fetch_codex(c: &reqwest::Client, folder: &Path) -> Result<Got, Fail> {
    let j: Value = std::fs::read(folder.join(".codex").join("auth.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let access = j["tokens"]["access_token"].as_str().ok_or(Fail::NotSignedIn)?;
    let account_id = j["tokens"]["account_id"].as_str().unwrap_or("").to_string();
    let d = get_json(c, "https://chatgpt.com/backend-api/codex/usage", |r| r.bearer_auth(access).header("chatgpt-account-id", account_id), Fail::Expired).await?;
    let mut rows = vec![];
    for w in [&d["rate_limit"]["primary_window"], &d["rate_limit"]["secondary_window"]] {
        if let Some(pct) = w["used_percent"].as_f64() {
            let reset = w["reset_at"].as_i64().map(|t| t.to_string()).unwrap_or_default();
            rows.push(Row { label: window_label(w["limit_window_seconds"].as_i64()), pct, resets_at: reset });
        }
    }
    Ok(Got { account: d["email"].as_str().unwrap_or("—").into(), plan: d["plan_type"].as_str().map(title_case).unwrap_or_default(), rows })
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

// ---- Orchestration ------------------------------------------------------------------------------------------

pub fn snapshot() -> Vec<Usage> {
    mem().last.values().cloned().collect()
}

/// Forget everything held for profiles that no longer exist.
fn prune(keep: &HashSet<String>) {
    let mut m = mem();
    m.last.retain(|k, _| keep.contains(k));
    m.limited.retain(|k, _| keep.contains(k));
}

/// 5-minute steps, doubling per consecutive 429, capped at an hour.
fn backoff_secs(strikes: u32) -> u64 {
    (300u64 << strikes.saturating_sub(1).min(4)).min(3600)
}

pub async fn refresh() -> Vec<Usage> {
    let Ok(client) = reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(20)).build() else { return snapshot() };
    let meta = store::load_meta();
    let mut jobs = vec![];
    let mut keep = HashSet::new();
    for m in &meta.profiles {
        let Some(app) = engine::app_by_key(&m.app).filter(|a| !a.is_terminal()) else { continue };
        let dir = store::data_dir().join(app.profiles_dirname).join(&m.folder);
        let k = key(app.key, &m.folder);
        keep.insert(k.clone());
        if !signin::signed_in(&app, &dir) {
            mem().last.remove(&k);
            continue;
        }
        let wait = mem().limited.get(&k).map(|l| l.1).filter(|&t| t > now());
        let (client, name, appkey) = (client.clone(), m.name.clone(), app.key);
        jobs.push(async move {
            let res = match (wait, appkey) {
                (Some(_), _) => Err(Fail::Limited),
                (None, "claude") => fetch_claude(&client, &k, &dir).await,
                _ => fetch_codex(&client, &dir).await,
            };
            (k, appkey, name, wait.is_some(), res)
        });
    }
    prune(&keep);
    for (k, app, name, skipped, res) in futures_util::future::join_all(jobs).await {
        let mut mm = mem();
        let mut u = mm.last.remove(&k).unwrap_or_else(|| Usage { app: app.into(), name: name.clone(), ..Default::default() });
        u.name = name;
        match res {
            Ok(g) => {
                (u.kind, u.message, u.account, u.plan, u.rows, u.updated) = (String::new(), String::new(), g.account, g.plan, g.rows, now());
                mm.limited.remove(&k);
            }
            Err(f) => {
                if matches!(f, Fail::Limited) && !skipped {
                    let strikes = mm.limited.get(&k).map_or(0, |l| l.0) + 1;
                    mm.limited.insert(k.clone(), (strikes, now() + backoff_secs(strikes)));
                }
                // The last good figures stay on screen.
                (u.kind, u.message) = { let (a, b) = f.parts(); (a.into(), b) };
            }
        }
        mm.last.insert(k, u);
    }
    snapshot()
}

// ---- Near-limit alerts --------------------------------------------------------------------------------------

pub struct Alert {
    pub title: String,
}

/// Which thresholds were newly crossed. Each (profile, window, reset time, level) fires once.
pub fn new_alerts(usage: &[Usage], running: impl Fn(&Usage) -> bool, notified: &mut HashSet<String>) -> Vec<Alert> {
    let mut out = vec![];
    for u in usage.iter().filter(|u| running(u)) {
        for r in &u.rows {
            let (level, title) = match r.label.as_str() {
                "5h" if r.pct >= 100.0 => ("limit", format!("{} hit its 5-hour limit", u.name)),
                "5h" if r.pct >= 85.0 => ("near", format!("{} is at {:.0}% of its 5-hour limit", u.name, r.pct)),
                "7d" if r.pct >= 90.0 => ("week", format!("{} is at {:.0}% of its weekly limit", u.name, r.pct)),
                _ => continue,
            };
            if notified.insert(format!("{}|{}|{}|{}|{level}", u.app, u.name, r.label, r.resets_at)) {
                out.push(Alert { title });
            }
        }
    }
    out
}

pub fn alerts_now(running: impl Fn(&Usage) -> bool) -> Vec<Alert> {
    let mut m = mem();
    let usage: Vec<Usage> = m.last.values().cloned().collect();
    new_alerts(&usage, running, &mut m.notified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::AeadCore;

    #[test]
    fn decrypts_v10_cookie_and_strips_host_hash() {
        let key = [7u8; 32];
        let c = Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = Aes256Gcm::generate_nonce(&mut aes_gcm::aead::OsRng);
        let mut plain = Sha256::digest(b".claude.ai").to_vec();
        plain.extend_from_slice(b"sk-ant-secret");
        let mut blob = b"v10".to_vec();
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&c.encrypt(&nonce, plain.as_slice()).unwrap());
        assert_eq!(decrypt_cookie(&key, ".claude.ai", &blob).unwrap(), "sk-ant-secret");
        assert!(decrypt_cookie(&[1u8; 32], ".claude.ai", &blob).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let enc = dpapi(b"hello", true).unwrap();
        assert_ne!(enc, b"hello");
        assert_eq!(dpapi(&enc, false).unwrap(), b"hello");
    }

    #[test]
    fn mac_cookie_cbc_round_trip() {
        use aes::cipher::BlockEncryptMut;
        let key = mac_key("hunter2");
        let mut plain = Sha256::digest(b".claude.ai").to_vec();
        plain.extend_from_slice(b"sk-ant-test");
        let enc = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &[b' '; 16]).unwrap().encrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&plain);
        let blob = [b"v10".as_slice(), &enc].concat();
        assert_eq!(decrypt_cookie(&key, ".claude.ai", &blob).as_deref(), Some("sk-ant-test"));
    }

    #[test]
    fn parses_claude_rows_and_picks_team_org() {
        let d: Value = serde_json::json!({
            "five_hour": {"utilization": 22.0, "resets_at": "2026-10-26T17:00:00Z"},
            "seven_day": {"utilization": 41.0},
            "limits": [{"kind": "weekly_scoped", "percent": 9.0, "scope": {"model": {"display_name": "Opus"}}}, {"kind": "other"}]
        });
        let rows = claude_rows(&d);
        assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["5h", "7d", "Opus"]);
        let orgs: Vec<Value> = serde_json::from_str(r#"[{"uuid":"a","capabilities":["api"]},{"uuid":"b","capabilities":["chat"]},{"uuid":"c","capabilities":["chat"],"raven_type":"team"}]"#).unwrap();
        assert_eq!(pick_org(&orgs).unwrap()["uuid"], "c");
        assert_eq!(account_label("me@x.com's Organization"), "me@x.com");
    }

    #[test]
    fn alerts_fire_once_per_window_and_only_for_running() {
        let u = Usage { app: "claude".into(), name: "Work".into(), rows: vec![Row { label: "5h".into(), pct: 90.0, resets_at: "t1".into() }], ..Default::default() };
        let mut seen = HashSet::new();
        assert_eq!(new_alerts(&[u.clone()], |_| true, &mut seen).len(), 1);
        assert!(new_alerts(&[u.clone()], |_| true, &mut seen).is_empty());
        let mut hit = u.clone();
        hit.rows[0].pct = 100.0;
        assert_eq!(new_alerts(&[hit], |_| true, &mut seen).len(), 1);
        assert!(new_alerts(&[u], |_| false, &mut HashSet::new()).is_empty());
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!([backoff_secs(1), backoff_secs(2), backoff_secs(9)], [300, 600, 3600]);
    }

    /// Manual: `cargo test live_refresh -- --ignored --nocapture` reads the real data folder (read-only).
    #[test]
    #[ignore]
    fn live_refresh() {
        for u in tauri::async_runtime::block_on(refresh()) {
            println!("{}/{} kind={:?} msg={:?} account={:?} plan={:?} updated={}", u.app, u.name, u.kind, u.message, u.account, u.plan, u.updated);
            for r in &u.rows {
                println!("    {} {}% resets {}", r.label, r.pct, r.resets_at);
            }
        }
    }
}

// ---- Used by the session scheduler ----------------------------------------------------------------------------

pub fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(120)).build().ok()
}

/// (Cookie header, organization id) for a Claude profile. Errors are fixed, user-facing text.
pub async fn claude_login(c: &reqwest::Client, app: &str, folder_name: &str, folder: &Path) -> Result<(String, String), String> {
    let cookie = claude_cookie(&key(app, folder_name), folder).await.map_err(|f| f.parts().1)?;
    let req = |r: reqwest::RequestBuilder| r.header("Cookie", cookie.clone()).header("Referer", "https://claude.ai/");
    let orgs = get_json(c, "https://claude.ai/api/organizations", req, Fail::Expired).await.map_err(|f| f.parts().1)?;
    let org = pick_org(orgs.as_array().map(Vec::as_slice).unwrap_or(&[])).ok_or("No organization found")?;
    let id = org["uuid"].as_str().or(org["id"].as_str()).ok_or("No organization found")?;
    Ok((cookie, id.to_string()))
}

pub fn http_fail(s: reqwest::StatusCode) -> String {
    status_fail(s, Fail::Expired).parts().1
}

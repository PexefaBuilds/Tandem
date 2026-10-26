//! Chat history: an on-disk SQLite full-text index of Claude Code and Codex CLI sessions (read from their
//! `.jsonl` files) and of desktop chats from data-export zips. Everything stays in Tandem's data folder.
use crate::{store, terminal};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

/// One message's text is cut here: tool output pasted into a chat can be megabytes.
const MAX_TEXT: usize = 20_000;
/// The preview shows this many messages; the list still counts them all.
const MAX_SHOWN: usize = 400;

static RUNNING: AtomicBool = AtomicBool::new(false);
static DONE: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub indexing: bool,
    pub done: usize,
    pub total: usize,
}

pub fn status() -> Status {
    Status { indexing: RUNNING.load(Ordering::Relaxed), done: DONE.load(Ordering::Relaxed), total: TOTAL.load(Ordering::Relaxed) }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    /// "cc" | "cx" | "claude" | "chatgpt"
    pub kind: String,
    pub profile: String,
    pub title: String,
    pub cwd: String,
    pub updated: i64,
    pub count: i64,
    pub snippet: String,
}

#[derive(Serialize)]
pub struct Message {
    pub role: String,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub row: SessionRow,
    pub messages: Vec<Message>,
    pub shown_all: bool,
}

#[derive(Default, Debug)]
pub struct Parsed {
    pub sid: String,
    pub cwd: String,
    pub title: String,
    pub started: i64,
    pub updated: i64,
    pub msgs: Vec<(String, String)>,
}

fn ts(v: &Value) -> i64 {
    match v {
        Value::String(s) => chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp()).unwrap_or(0),
        Value::Number(n) => n.as_f64().unwrap_or(0.0) as i64,
        _ => 0,
    }
}

fn clip(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

/// Text of a message's content: a plain string, or the text blocks of a list (tool calls and results are left out).
fn content_text(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter(|b| matches!(b["type"].as_str(), Some("text" | "input_text" | "output_text")))
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Command wrappers and injected context aren't something the person typed or read.
fn noise(t: &str) -> bool {
    let t = t.trim_start();
    t.is_empty() || t.starts_with("<command-") || t.starts_with("<local-command") || t.starts_with("<environment_context") || t.starts_with("<system-reminder") || t.starts_with("Caveat:")
}

pub fn parse_cc(reader: impl BufRead) -> Parsed {
    let mut p = Parsed::default();
    for line in reader.lines().map_while(Result::ok) {
        let wanted = line.contains("\"type\":\"user\"") || line.contains("\"type\":\"assistant\"") || line.contains("\"custom-title\"");
        if !wanted {
            continue;
        }
        let Ok(j) = serde_json::from_str::<Value>(&line) else { continue };
        if j["type"] == "custom-title" {
            if let Some(t) = j["customTitle"].as_str() {
                p.title = t.to_string();
            }
            continue;
        }
        if j["isSidechain"] == true || j["isMeta"] == true {
            continue;
        }
        let role = match j["type"].as_str() {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };
        let text = content_text(&j["message"]["content"]);
        if noise(&text) {
            continue;
        }
        if p.sid.is_empty() {
            p.sid = j["sessionId"].as_str().unwrap_or("").to_string();
        }
        if p.cwd.is_empty() {
            p.cwd = j["cwd"].as_str().unwrap_or("").to_string();
        }
        let t = ts(&j["timestamp"]);
        if p.started == 0 {
            p.started = t;
        }
        p.updated = p.updated.max(t);
        p.msgs.push((role.into(), clip(&text)));
    }
    finish_title(&mut p);
    p
}

pub fn parse_cx(reader: impl BufRead) -> Parsed {
    let mut p = Parsed::default();
    for line in reader.lines().map_while(Result::ok) {
        if !(line.contains("\"session_meta\"") || line.contains("\"type\":\"message\"")) {
            continue;
        }
        let Ok(j) = serde_json::from_str::<Value>(&line) else { continue };
        let pl = &j["payload"];
        if j["type"] == "session_meta" {
            p.sid = pl["id"].as_str().unwrap_or("").to_string();
            p.cwd = pl["cwd"].as_str().unwrap_or("").to_string();
            continue;
        }
        if pl["type"] != "message" {
            continue;
        }
        let role = match pl["role"].as_str() {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };
        let text = content_text(&pl["content"]);
        if noise(&text) {
            continue;
        }
        let t = ts(&j["timestamp"]);
        if p.started == 0 {
            p.started = t;
        }
        p.updated = p.updated.max(t);
        p.msgs.push((role.into(), clip(&text)));
    }
    finish_title(&mut p);
    p
}

fn finish_title(p: &mut Parsed) {
    if p.title.is_empty() {
        let first = p.msgs.iter().find(|m| m.0 == "user").map(|m| m.1.as_str()).unwrap_or("Untitled");
        p.title = first.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect();
    }
}

/// Claude and ChatGPT data exports both ship a `conversations.json`; the shapes differ.
/// Returns (kind, parsed) per conversation.
pub fn parse_export(bytes: &[u8]) -> Vec<(String, Parsed)> {
    let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(bytes) else { return vec![] };
    let mut out = vec![];
    for c in items {
        if c["mapping"].is_object() {
            // ChatGPT: a tree of messages. Walk from the last node up to the root, then reverse.
            let mut p = Parsed { sid: c["id"].as_str().or(c["conversation_id"].as_str()).unwrap_or("").into(), title: c["title"].as_str().unwrap_or("").into(), started: ts(&c["create_time"]), updated: ts(&c["update_time"]), ..Default::default() };
            let mut chain = vec![];
            let mut node = c["current_node"].as_str().map(String::from);
            while let Some(id) = node {
                let n = &c["mapping"][&id];
                chain.push(n.clone());
                node = n["parent"].as_str().map(String::from);
                if chain.len() > 100_000 {
                    break;
                }
            }
            for n in chain.iter().rev() {
                let m = &n["message"];
                let role = match m["author"]["role"].as_str() {
                    Some("user") => "user",
                    Some("assistant") => "assistant",
                    _ => continue,
                };
                let text = m["content"]["parts"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                if !text.trim().is_empty() {
                    p.msgs.push((role.into(), clip(&text)));
                }
            }
            if !p.msgs.is_empty() {
                finish_title(&mut p);
                out.push(("chatgpt".to_string(), p));
            }
        } else if c["chat_messages"].is_array() {
            let mut p = Parsed { sid: c["uuid"].as_str().unwrap_or("").into(), title: c["name"].as_str().unwrap_or("").into(), started: ts(&c["created_at"]), updated: ts(&c["updated_at"]), ..Default::default() };
            for m in c["chat_messages"].as_array().into_iter().flatten() {
                let role = if m["sender"] == "human" { "user" } else { "assistant" };
                let mut text = content_text(&m["content"]);
                if text.trim().is_empty() {
                    text = m["text"].as_str().unwrap_or("").to_string();
                }
                if !text.trim().is_empty() {
                    p.msgs.push((role.into(), clip(&text)));
                }
            }
            if !p.msgs.is_empty() {
                finish_title(&mut p);
                out.push(("claude".to_string(), p));
            }
        }
    }
    out
}

/// `"word"*` for each word, so typing part of a word still finds it. None when there is nothing to search for.
pub fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn db() -> Result<Connection, String> {
    let dir = store::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_db(&dir.join("history.db"))
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let c = Connection::open(path).map_err(|e| e.to_string())?;
    c.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, kind TEXT, profile TEXT, title TEXT, cwd TEXT, sid TEXT, root TEXT,
            started INT, updated INT, count INT, path TEXT, mtime INT, size INT);
         CREATE TABLE IF NOT EXISTS messages(id INTEGER PRIMARY KEY, session TEXT, idx INT, role TEXT, text TEXT);
         CREATE INDEX IF NOT EXISTS messages_session ON messages(session, idx);
         CREATE VIRTUAL TABLE IF NOT EXISTS ft USING fts5(text, content='messages', content_rowid='id');",
    )
    .map_err(|e| e.to_string())?;
    Ok(c)
}

fn remove_session(c: &Connection, id: &str) -> rusqlite::Result<()> {
    c.execute("INSERT INTO ft(ft, rowid, text) SELECT 'delete', id, text FROM messages WHERE session=?1", [id])?;
    c.execute("DELETE FROM messages WHERE session=?1", [id])?;
    c.execute("DELETE FROM sessions WHERE id=?1", [id])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn save_session(c: &mut Connection, id: &str, kind: &str, profile: &str, root: &str, path: &str, mtime: i64, size: i64, p: &Parsed) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    remove_session(&tx, id)?;
    tx.execute(
        "INSERT INTO sessions(id,kind,profile,title,cwd,sid,root,started,updated,count,path,mtime,size) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![id, kind, profile, p.title, p.cwd, p.sid, root, p.started, p.updated.max(p.started), p.msgs.len() as i64, path, mtime, size],
    )?;
    for (i, (role, text)) in p.msgs.iter().enumerate() {
        tx.execute("INSERT INTO messages(session,idx,role,text) VALUES(?1,?2,?3,?4)", params![id, i as i64, role, text])?;
        let rid = tx.last_insert_rowid();
        tx.execute("INSERT INTO ft(rowid,text) VALUES(?1,?2)", params![rid, text])?;
    }
    tx.commit()
}

struct Source {
    kind: &'static str,
    profile: String,
    root: PathBuf,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

fn sources() -> Vec<Source> {
    let mut v = vec![];
    if let Some(h) = home() {
        v.push(Source { kind: "cc", profile: "Default".into(), root: h.join(".claude") });
        v.push(Source { kind: "cx", profile: "Default".into(), root: h.join(".codex") });
    }
    for m in store::load_meta().profiles {
        if let Some(a) = crate::engine::app_by_key(&m.app).filter(|a| a.is_terminal()) {
            v.push(Source { kind: a.key, profile: m.name.clone(), root: store::data_dir().join(a.profiles_dirname).join(&m.folder) });
        }
    }
    v
}

fn jsonl_files(s: &Source) -> Vec<PathBuf> {
    let mut out = vec![];
    if s.kind == "cc" {
        // projects/<project>/<session>.jsonl only: deeper files are sub-agent transcripts.
        for proj in std::fs::read_dir(s.root.join("projects")).into_iter().flatten().flatten() {
            for f in std::fs::read_dir(proj.path()).into_iter().flatten().flatten() {
                if f.path().extension().is_some_and(|e| e == "jsonl") {
                    out.push(f.path());
                }
            }
        }
    } else {
        fn walk(d: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl")) {
                    out.push(p);
                }
            }
        }
        walk(&s.root.join("sessions"), &mut out);
    }
    out
}

/// Reads files that are new or changed since the last pass, and forgets sessions whose files are gone.
pub fn refresh() -> Result<(), String> {
    let Ok(_g) = LOCK.try_lock() else { return Ok(()) };
    RUNNING.store(true, Ordering::Relaxed);
    let res = refresh_inner();
    RUNNING.store(false, Ordering::Relaxed);
    res
}

fn refresh_inner() -> Result<(), String> {
    let mut c = db()?;
    let srcs = sources();
    let work: Vec<(usize, PathBuf)> = srcs.iter().enumerate().flat_map(|(i, s)| jsonl_files(s).into_iter().map(move |f| (i, f))).collect();
    DONE.store(0, Ordering::Relaxed);
    TOTAL.store(work.len(), Ordering::Relaxed);
    let mut seen = std::collections::HashSet::new();
    for (i, f) in &work {
        let s = &srcs[*i];
        let id = format!("{}:{}", s.kind, f.display());
        seen.insert(id.clone());
        DONE.fetch_add(1, Ordering::Relaxed);
        let Ok(meta) = f.metadata() else { continue };
        let (mtime, size) = (meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64), meta.len() as i64);
        let same: bool = c.query_row("SELECT mtime=?2 AND size=?3 FROM sessions WHERE id=?1", params![id, mtime, size], |r| r.get(0)).unwrap_or(false);
        if same {
            continue;
        }
        let Ok(file) = std::fs::File::open(f) else { continue };
        let r = BufReader::new(file);
        let mut p = if s.kind == "cc" { parse_cc(r) } else { parse_cx(r) };
        if p.msgs.is_empty() {
            let _ = remove_session(&c, &id);
            continue;
        }
        if p.sid.is_empty() {
            p.sid = f.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
        }
        if p.updated == 0 {
            p.updated = mtime;
        }
        save_session(&mut c, &id, s.kind, &s.profile, &s.root.to_string_lossy(), &f.to_string_lossy(), mtime, size, &p).map_err(|e| e.to_string())?;
    }
    // Sessions read from files that no longer exist (imported exports have no path on disk and stay).
    let gone: Vec<String> = {
        let mut st = c.prepare("SELECT id FROM sessions WHERE kind IN ('cc','cx')").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.flatten().filter(|id| !seen.contains(id)).collect()
    };
    for id in gone {
        let _ = remove_session(&c, &id);
    }
    Ok(())
}

/// About 120 characters of `text` around the first word of the search that appears in it.
fn snippet(text: &str, query: &str) -> String {
    let lower = text.to_lowercase();
    let at = query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).filter_map(|w| lower.find(&w.to_lowercase())).min().unwrap_or(0);
    // `lower` can differ in length from `text` for some characters, so cut on char counts, not byte offsets.
    let chars: Vec<char> = text.chars().collect();
    let pos = lower[..at.min(lower.len())].chars().count().min(chars.len());
    let (from, to) = (pos.saturating_sub(40), (pos + 80).min(chars.len()));
    let body: String = chars[from..to].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{body}{}", if from > 0 { "…" } else { "" }, if to < chars.len() { "…" } else { "" })
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow { id: r.get(0)?, kind: r.get(1)?, profile: r.get(2)?, title: r.get(3)?, cwd: r.get(4)?, updated: r.get(5)?, count: r.get(6)?, snippet: String::new() })
}

/// `filter` is "all" or a kind. `query` is what the person typed.
pub fn list(query: &str, filter: &str) -> Result<Vec<SessionRow>, String> {
    list_in(&db()?, query, filter)
}

fn list_in(c: &Connection, query: &str, filter: &str) -> Result<Vec<SessionRow>, String> {
    let kind = if filter == "all" { "%" } else { filter };
    let cols = "s.id,s.kind,s.profile,s.title,s.cwd,s.updated,s.count";
    let mut out = vec![];
    match fts_query(query) {
        None => {
            let mut st = c.prepare(&format!("SELECT {cols} FROM sessions s WHERE s.kind LIKE ?1 ORDER BY s.updated DESC LIMIT 300")).map_err(|e| e.to_string())?;
            for r in st.query_map([kind], row_from).map_err(|e| e.to_string())? {
                out.push(r.map_err(|e| e.to_string())?);
            }
        }
        Some(q) => {
            // One row per session: the newest matching session first, with a snippet from one match.
            let mut st = c
                .prepare(&format!(
                    "SELECT {cols}, m.text FROM ft JOIN messages m ON m.id=ft.rowid JOIN sessions s ON s.id=m.session
                     WHERE ft MATCH ?1 AND s.kind LIKE ?2 GROUP BY s.id ORDER BY s.updated DESC LIMIT 300"
                ))
                .map_err(|e| e.to_string())?;
            let rows = st.query_map(params![q, kind], |r| row_from(r).map(|mut x| { x.snippet = snippet(&r.get::<_, String>(7).unwrap_or_default(), query); x }));
            for r in rows.map_err(|e| e.to_string())? {
                out.push(r.map_err(|e| e.to_string())?);
            }
        }
    }
    Ok(out)
}

pub fn detail(id: &str) -> Result<Detail, String> {
    let c = db()?;
    let row = c.query_row("SELECT id,kind,profile,title,cwd,updated,count FROM sessions WHERE id=?1", [id], row_from).map_err(|_| "That chat is no longer indexed.".to_string())?;
    let mut st = c.prepare("SELECT role,text FROM messages WHERE session=?1 ORDER BY idx LIMIT ?2").map_err(|e| e.to_string())?;
    let messages: Vec<Message> = st.query_map(params![id, MAX_SHOWN as i64], |r| Ok(Message { role: r.get(0)?, text: r.get(1)? })).map_err(|e| e.to_string())?.flatten().collect();
    let shown_all = (messages.len() as i64) >= row.count;
    Ok(Detail { row, messages, shown_all })
}

/// Reads an export zip and stores its conversations under `profile`. Returns how many were added.
pub fn import_export(zip_path: &Path, app: &str, profile: &str) -> Result<usize, String> {
    let file = std::fs::File::open(zip_path).map_err(|_| "Couldn't open that file.".to_string())?;
    let mut z = zip::ZipArchive::new(file).map_err(|_| "That isn't a zip file.".to_string())?;
    let mut bytes = vec![];
    z.by_name("conversations.json").map_err(|_| "No conversations.json in that zip. Is it a data export?".to_string())?.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let convs = parse_export(&bytes);
    let want = if app == "codex" { "chatgpt" } else { "claude" };
    let convs: Vec<_> = convs.into_iter().filter(|(k, _)| k == want).collect();
    if convs.is_empty() {
        return Err(format!("No {} conversations found in that export.", if want == "chatgpt" { "ChatGPT" } else { "Claude" }));
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = db()?;
    let n = convs.len();
    for (kind, p) in convs {
        let id = format!("{kind}:{profile}:{}", p.sid);
        save_session(&mut c, &id, &kind, profile, "", "", 0, 0, &p).map_err(|e| e.to_string())?;
    }
    Ok(n)
}

/// Desktop profiles that have at least one imported chat, and how many desktop profiles exist.
pub fn export_coverage() -> (usize, usize) {
    let total = store::load_meta().profiles.iter().filter(|m| m.app == "claude" || m.app == "codex").count();
    let done = db().ok().and_then(|c| c.query_row("SELECT COUNT(DISTINCT kind||profile) FROM sessions WHERE kind IN ('claude','chatgpt')", [], |r| r.get::<_, i64>(0)).ok()).unwrap_or(0) as usize;
    (done, total)
}

/// Opens the session in a terminal: `claude --resume <id>` or `codex resume <id>`, in the session's folder.
pub fn resume(id: &str, s: &store::Settings) -> Result<(), String> {
    let c = db()?;
    let (kind, sid, cwd, root): (String, String, String, String) = c
        .query_row("SELECT kind,sid,cwd,root FROM sessions WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|_| "That chat is no longer indexed.".to_string())?;
    let app = crate::engine::app_by_key(&kind).filter(|a| a.is_terminal()).ok_or("Only terminal sessions can be resumed.")?;
    if !sid.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
        return Err("Unexpected session id.".into());
    }
    terminal::resume(&app, Path::new(&root), s, &sid, if cwd.is_empty() { None } else { Some(&cwd) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn claude_code_lines_keep_only_chat_text() {
        let src = r#"{"type":"custom-title","customTitle":"My chat","sessionId":"s1"}
{"type":"user","sessionId":"s1","cwd":"C:\\p","timestamp":"2026-10-11T17:18:16.603Z","message":{"content":"<command-name>/x</command-name>"}}
{"type":"user","sessionId":"s1","cwd":"C:\\p","timestamp":"2026-10-11T17:18:17.000Z","message":{"content":"hello there"}}
{"type":"user","sessionId":"s1","message":{"content":[{"type":"tool_result","content":"big"}]}}
{"type":"assistant","sessionId":"s1","timestamp":"2026-10-11T17:18:20.000Z","message":{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"hi back"}]}}
{"type":"attachment","x":1}"#;
        let p = parse_cc(Cursor::new(src));
        assert_eq!((p.sid.as_str(), p.title.as_str(), p.cwd.as_str()), ("s1", "My chat", "C:\\p"));
        assert_eq!(p.msgs, vec![("user".to_string(), "hello there".to_string()), ("assistant".to_string(), "hi back".to_string())]);
        assert!(p.updated > p.started);
    }

    #[test]
    fn codex_rollouts_and_exports() {
        let src = r#"{"timestamp":"2026-10-11T10:00:00Z","type":"session_meta","payload":{"id":"abc","cwd":"D:\\w"}}
{"timestamp":"2026-10-11T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>x</environment_context>"}]}}
{"timestamp":"2026-10-11T10:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"fix the bug"}]}}
{"timestamp":"2026-10-11T10:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#;
        let p = parse_cx(Cursor::new(src));
        assert_eq!((p.sid.as_str(), p.msgs.len(), p.title.as_str()), ("abc", 2, "fix the bug"));

        let claude = br#"[{"uuid":"u1","name":"Trip","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z","chat_messages":[{"sender":"human","text":"plan a trip"},{"sender":"assistant","text":"sure"}]}]"#;
        let r = parse_export(claude);
        assert_eq!((r[0].0.as_str(), r[0].1.msgs.len()), ("claude", 2));
        let gpt = br#"[{"id":"c1","title":"T","create_time":1.0,"update_time":2.0,"current_node":"b","mapping":{"a":{"parent":null,"message":{"author":{"role":"user"},"content":{"parts":["q"]}}},"b":{"parent":"a","message":{"author":{"role":"assistant"},"content":{"parts":["a"]}}}}}]"#;
        let g = parse_export(gpt);
        assert_eq!((g[0].0.as_str(), g[0].1.msgs[0].1.as_str()), ("chatgpt", "q"));
    }

    #[test]
    fn search_text_becomes_prefix_terms() {
        assert_eq!(fts_query("fix  bug!").as_deref(), Some("\"fix\"* \"bug\"*"));
        assert_eq!(fts_query("  ?! "), None);
    }

    #[test]
    fn index_search_and_remove() {
        let dir = std::env::temp_dir().join(format!("tandem-hist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut c = open_db(&dir.join("h.db")).unwrap();
        let p = Parsed { sid: "s".into(), title: "T".into(), updated: 5, started: 1, msgs: vec![("user".into(), "needle in a haystack".into())], ..Default::default() };
        save_session(&mut c, "cc:x", "cc", "Default", "", "x", 0, 0, &p).unwrap();
        assert_eq!(list_in(&c, "need", "all").unwrap().len(), 1);
        assert_eq!(list_in(&c, "need", "cx").unwrap().len(), 0);
        assert_eq!(list_in(&c, "", "cc").unwrap().len(), 1);
        remove_session(&c, "cc:x").unwrap();
        assert_eq!(list_in(&c, "needle", "all").unwrap().len(), 0);
        drop(c);
        let _ = std::fs::remove_dir_all(dir);
    }
}

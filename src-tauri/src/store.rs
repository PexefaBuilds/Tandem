//! Profiles and settings on disk. `TANDEM_DATA_DIR` overrides the data folder (use it for testing).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());
pub fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TANDEM_DATA_DIR") {
        return d.into();
    }
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join("Library/Application Support").into()))
        .unwrap_or_default();
    Path::new(&base).join("Tandem")
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Meta {
    pub app: String,
    pub name: String,
    /// Folder name on disk; differs from `name` after a rename until the next launch.
    pub folder: String,
    pub tint: String,
    /// Unix seconds of the last launch; 0 = never.
    #[serde(default)]
    pub last_opened: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct MetaFile {
    pub profiles: Vec<Meta>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "minimize" | "hide" | "stay" | "quit" ("tray" is the old name for "minimize")
    pub after_launch: String,
    pub welcome_done: bool,
    /// Minutes between usage refreshes; 0 = manual only.
    pub refresh_minutes: u32,
    pub warn_near_limit: bool,
    pub show_tray: bool,
    pub open_at_login: bool,
    /// "wt" | "powershell"
    pub terminal: String,
    /// Clear API-key variables in terminal profiles so each uses its own sign-in.
    pub keep_separate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { after_launch: "minimize".into(), welcome_done: false, refresh_minutes: 5, warn_near_limit: true, show_tray: true, open_at_login: false, terminal: "wt".into(), keep_separate: true }
    }
}

fn read<T: for<'a> Deserialize<'a> + Default>(file: &str) -> T {
    std::fs::read_to_string(data_dir().join(file)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

// Write to a temp file and rename so a crash never leaves half a file.
fn write<T: Serialize>(file: &str, v: &T) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{file}.tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, dir.join(file)).map_err(|e| e.to_string())
}

pub fn load_meta() -> MetaFile {
    read("profiles.json")
}
pub fn save_meta(m: &MetaFile) -> Result<(), String> {
    write("profiles.json", m)
}
pub fn load_settings() -> Settings {
    read("settings.json")
}
pub fn save_settings(s: &Settings) -> Result<(), String> {
    write("settings.json", s)
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Member {
    pub app: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Group {
    pub name: String,
    pub members: Vec<Member>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct GroupFile {
    pub groups: Vec<Group>,
}

pub fn load_groups() -> GroupFile {
    read("groups.json")
}
pub fn save_groups(g: &GroupFile) -> Result<(), String> {
    write("groups.json", g)
}

/// Keep groups in step when a profile is renamed (`to` Some) or deleted (`to` None).
pub fn group_member_changed(app: &str, name: &str, to: Option<&str>) {
    let mut g = load_groups();
    for grp in &mut g.groups {
        grp.members = grp
            .members
            .drain(..)
            .filter_map(|mut m| {
                if m.app == app && m.name == name {
                    m.name = to?.to_string();
                }
                Some(m)
            })
            .collect();
    }
    let _ = save_groups(&g);
}

/// BETA session start for one Claude profile (see `schedule.rs`).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    pub app: String,
    pub name: String,
    pub enabled: bool,
    /// "HH:MM", local time.
    pub time: String,
    /// "daily" | "once"
    pub repeat: String,
    pub message: String,
    /// "terminal" (Claude Code CLI) | "web" (claude.ai route). Empty = web, from before this option existed.
    pub route: String,
    /// Name of the Claude Code profile used by the terminal route.
    pub terminal: String,
    /// Unix seconds of the last message that went through; 0 = never.
    pub last_sent: u64,
    /// "" | "sent" | "failed" | "missed"
    pub last_status: String,
    pub last_note: String,
    /// Local date (YYYY-MM-DD) the schedule last fired or was marked missed.
    pub last_day: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ScheduleFile {
    pub schedules: Vec<Schedule>,
}

pub fn load_schedules() -> ScheduleFile {
    read("schedules.json")
}
pub fn save_schedules(f: &ScheduleFile) -> Result<(), String> {
    write("schedules.json", f)
}

/// Keep a profile's schedule in step when it is renamed (`to` Some) or deleted (`to` None).
pub fn schedule_member_changed(app: &str, name: &str, to: Option<&str>) {
    let mut f = load_schedules();
    match to {
        Some(n) => f.schedules.iter_mut().filter(|s| s.app == app && s.name == name).for_each(|s| s.name = n.to_string()),
        None => f.schedules.retain(|s| !(s.app == app && s.name == name)),
    }
    let _ = save_schedules(&f);
}

const TINTS: [&str; 6] = ["graphite", "blue", "green", "amber", "rose", "teal"];
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Returns the trimmed name. `current` is the profile's own name when editing.
pub fn validate_name(meta: &MetaFile, app: &str, name: &str, current: Option<&str>) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name.chars().count() > 40 {
        return Err("Use 40 characters or fewer.".into());
    }
    if name.chars().any(|c| "<>:\"/\\|?*".contains(c) || c.is_control()) {
        return Err("Names can't contain < > : \" / \\ | ? *".into());
    }
    if name.ends_with('.') || RESERVED.contains(&name.to_lowercase().as_str()) || name.eq_ignore_ascii_case("main instance") {
        return Err("That name isn't allowed.".into());
    }
    let clash = meta.profiles.iter().any(|m| m.app == app && m.name.eq_ignore_ascii_case(name) && Some(m.name.as_str()) != current);
    if clash {
        return Err("You already have a profile with this name.".into());
    }
    Ok(name.to_string())
}

pub fn valid_tint(t: &str) -> bool {
    TINTS.contains(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> MetaFile {
        MetaFile { profiles: vec![Meta { app: "claude".into(), name: "Work".into(), folder: "Work".into(), tint: "blue".into(), last_opened: 0 }] }
    }

    #[test]
    fn rejects_bad_names() {
        let m = meta();
        for bad in ["", "  ", "a/b", "x*", "CON", "dot.", "Main instance", &"a".repeat(41)] {
            assert!(validate_name(&m, "claude", bad, None).is_err(), "{bad}");
        }
    }

    #[test]
    fn unique_per_app_case_insensitive_but_allows_self() {
        let m = meta();
        assert!(validate_name(&m, "claude", "work", None).is_err());
        assert!(validate_name(&m, "codex", "work", None).is_ok());
        assert_eq!(validate_name(&m, "claude", " Work ", Some("Work")).unwrap(), "Work");
    }
}

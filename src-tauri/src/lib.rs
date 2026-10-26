mod engine;
mod history;
mod import;
mod platform;
mod schedule;
mod shortcut;
mod signin;
mod store;
mod terminal;
mod tray;
mod updater;
mod usage;

use engine::{app_by_key, App};
use serde::Serialize;
use std::path::PathBuf;
use store::{Meta, Settings};

type Res<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    key: &'static str,
    name: &'static str,
    installed: bool,
    version: String,
    path: String,
    main_running: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileInfo {
    app: &'static str,
    name: String,
    tint: String,
    folder: String,
    running: bool,
    signed_in: bool,
    has_shortcut: bool,
    last_opened: u64,
    account: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct State {
    apps: Vec<AppInfo>,
    profiles: Vec<ProfileInfo>,
    settings: Settings,
    data_dir: String,
    schedules: Vec<store::Schedule>,
}

fn profiles_dir(app: &App) -> PathBuf {
    store::data_dir().join(app.profiles_dirname)
}

fn app_of(key: &str) -> Res<App> {
    app_by_key(key).ok_or_else(|| format!("Unknown app: {key}"))
}

#[tauri::command]
async fn get_state() -> Res<State> {
    let meta = store::load_meta();
    let mut apps = vec![];
    let mut profiles = vec![];
    for app in [engine::CLAUDE, engine::CODEX, engine::CLAUDE_CODE, engine::CODEX_CLI] {
        if app.is_terminal() {
            let tool = terminal::detect(&app);
            apps.push(AppInfo { key: app.key, name: app.display, installed: tool.is_some(), version: tool.as_ref().map(|t| t.version.clone()).unwrap_or_default(), path: tool.map(|t| t.path).unwrap_or_default(), main_running: false });
            for m in meta.profiles.iter().filter(|m| m.app == app.key) {
                let dir = profiles_dir(&app).join(&m.folder);
                profiles.push(ProfileInfo {
                    app: app.key,
                    name: m.name.clone(),
                    tint: m.tint.clone(),
                    folder: dir.display().to_string(),
                    running: false,
                    signed_in: terminal::signed_in(&app, &dir),
                    has_shortcut: shortcut::exists(&app, &m.name),
                    last_opened: m.last_opened,
                    account: terminal::account(&app, &dir),
                });
            }
            continue;
        }
        let exe = engine::find_exe(&app);
        let run = engine::running(&app, &profiles_dir(&app));
        apps.push(AppInfo {
            key: app.key,
            name: app.display,
            installed: exe.is_some(),
            version: exe.as_deref().map(engine::app_version).unwrap_or_default(),
            path: exe.map(|p| p.display().to_string()).unwrap_or_default(),
            main_running: run.main,
        });
        for m in meta.profiles.iter().filter(|m| m.app == app.key) {
            profiles.push(ProfileInfo {
                app: app.key,
                name: m.name.clone(),
                tint: m.tint.clone(),
                folder: profiles_dir(&app).join(&m.folder).display().to_string(),
                running: run.profiles.keys().any(|k| k.eq_ignore_ascii_case(&m.folder)),
                signed_in: signin::signed_in(&app, &profiles_dir(&app).join(&m.folder)),
                has_shortcut: shortcut::exists(&app, &m.name),
                last_opened: m.last_opened,
                account: String::new(),
            });
        }
    }
    Ok(State { apps, profiles, settings: store::load_settings(), data_dir: store::data_dir().display().to_string(), schedules: store::load_schedules().schedules })
}

#[tauri::command]
fn create_profile(app: String, name: String, tint: String) -> Res<()> {
    let a = app_of(&app)?;
    let _g = store::lock();
    let mut meta = store::load_meta();
    let name = store::validate_name(&meta, a.key, &name, None)?;
    if !store::valid_tint(&tint) { return Err("Unknown colour.".into()); }
    std::fs::create_dir_all(profiles_dir(&a).join(&name)).map_err(|e| e.to_string())?;
    if a.is_terminal() {
        terminal::prepare(&a, &profiles_dir(&a).join(&name))?;
    }
    meta.profiles.push(Meta { app: a.key.into(), folder: name.clone(), name, tint, last_opened: 0 });
    store::save_meta(&meta)
}

#[tauri::command]
fn update_profile(app: String, name: String, new_name: String, tint: String) -> Res<()> {
    let a = app_of(&app)?;
    let _g = store::lock();
    let mut meta = store::load_meta();
    let new_name = store::validate_name(&meta, a.key, &new_name, Some(&name))?;
    if !store::valid_tint(&tint) { return Err("Unknown colour.".into()); }
    let m = meta.profiles.iter_mut().find(|m| m.app == a.key && m.name == name).ok_or("Profile not found")?;
    // The folder is renamed on the next launch, so a running profile can be edited.
    let had_shortcut = shortcut::exists(&a, &name);
    let renamed = m.name != new_name;
    m.name = new_name.clone();
    m.tint = tint;
    store::save_meta(&meta)?;
    if renamed {
        store::group_member_changed(a.key, &name, Some(&new_name));
        store::schedule_member_changed(a.key, &name, Some(&new_name));
        if had_shortcut {
            shortcut::remove(&a, &name);
            let _ = shortcut::create(&a, &new_name);
        }
    }
    Ok(())
}

#[tauri::command]
async fn delete_profile(app: String, name: String) -> Res<()> {
    let a = app_of(&app)?;
    let _g = store::lock();
    let mut meta = store::load_meta();
    let pos = meta.profiles.iter().position(|m| m.app == a.key && m.name == name).ok_or("Profile not found")?;
    let folder = meta.profiles[pos].folder.clone();
    let pids = if a.is_terminal() { Default::default() } else { engine::running(&a, &profiles_dir(&a)).profiles };
    if let Some(p) = pids.iter().find(|(k, _)| k.eq_ignore_ascii_case(&folder)) {
        engine::kill_pids(p.1);
    }
    engine::remove_dir_retry(&profiles_dir(&a).join(&folder))?;
    usage::forget(a.key, &folder);
    shortcut::remove(&a, &name);
    store::group_member_changed(a.key, &name, None);
    store::schedule_member_changed(a.key, &name, None);
    meta.profiles.remove(pos);
    store::save_meta(&meta)
}

/// `name` None launches the app's Main instance.
#[tauri::command]
async fn launch(app: String, name: Option<String>) -> Res<()> {
    launch_profile(&app, name)
}

fn launch_profile(app: &str, name: Option<String>) -> Res<()> {
    let a = app_of(app)?;
    let Some(name) = name else { return engine::launch_main(&a) };
    let _g = store::lock();
    let mut meta = store::load_meta();
    let m = meta.profiles.iter_mut().find(|m| m.app == a.key && m.name == name).ok_or("Profile not found")?;
    let root = profiles_dir(&a);
    if m.folder != m.name && !root.join(&m.name).exists() {
        std::fs::rename(root.join(&m.folder), root.join(&m.name)).map_err(|e| format!("Could not rename the profile folder: {e}"))?;
        m.folder = m.name.clone();
    }
    let folder = root.join(&m.folder);
    m.last_opened = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let pname = m.name.clone();
    store::save_meta(&meta)?;
    if a.is_terminal() {
        return terminal::open(&a, &pname, &folder, &store::load_settings(), false);
    }
    engine::launch(&a, &folder)
}

#[tauri::command]
async fn close(app: String, name: Option<String>) -> Res<usize> {
    let a = app_of(&app)?;
    if a.is_terminal() {
        return Ok(0);
    }
    let run = engine::running(&a, &profiles_dir(&a));
    let pids: Vec<u32> = match name {
        None => engine::main_pids_from(&engine::processes(&a), &a),
        Some(n) => {
            let folder = store::load_meta().profiles.iter().find(|m| m.app == a.key && m.name == n).map(|m| m.folder.clone()).ok_or("Profile not found")?;
            run.profiles.into_iter().find(|(k, _)| k.eq_ignore_ascii_case(&folder)).map(|(_, v)| v).unwrap_or_default()
        }
    };
    Ok(engine::kill_pids(&pids))
}

#[tauri::command]
fn show_folder(app: String, name: Option<String>) -> Res<()> {
    let a = app_of(&app)?;
    let dir = match name {
        Some(n) => {
            let folder = store::load_meta().profiles.iter().find(|m| m.app == a.key && m.name == n).map(|m| m.folder.clone()).ok_or("Profile not found")?;
            profiles_dir(&a).join(folder)
        }
        None => profiles_dir(&a),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener).arg(dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Found {
    path: String,
    name: String,
    app: &'static str,
    has_sign_in: bool,
    in_use: bool,
    is_default: bool,
}

/// True when the profile is open: its --user-data-dir is in use, or it is the Main instance's default folder
/// and the Main instance is running.
fn in_use(app: &App, path: &std::path::Path, dirs: &[String]) -> bool {
    let key = norm(path);
    dirs.contains(&key)
        || (signin::default_slot_dir().is_some_and(|d| norm(&d) == key) && !engine::main_pids_from(&engine::processes(app), app).is_empty())
}

fn found(items: Vec<(PathBuf, &'static str, bool)>) -> Vec<Found> {
    let ours = store::data_dir();
    let dirs: std::collections::HashMap<&str, Vec<String>> = [engine::CLAUDE, engine::CODEX].iter().map(|a| (a.key, engine::user_data_dirs(a))).collect();
    items
        .into_iter()
        .filter(|(p, _, _)| !p.starts_with(&ours))
        .map(|(p, app, is_default)| Found {
            name: if is_default { "Main account".into() } else { p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() },
            has_sign_in: import::has_sign_in_data(&p),
            in_use: in_use(&app_by_key(app).unwrap(), &p, &dirs[app]),
            path: p.display().to_string(),
            app,
            is_default,
        })
        .collect()
}

/// Whether each listed profile is open right now: a quick re-check, without scanning the disk again.
#[tauri::command]
async fn recheck_in_use(items: Vec<(String, String)>) -> Vec<bool> {
    let dirs: std::collections::HashMap<&str, Vec<String>> = [engine::CLAUDE, engine::CODEX].iter().map(|a| (a.key, engine::user_data_dirs(a))).collect();
    items.iter().map(|(p, app)| app_by_key(app).zip(dirs.get(app.as_str())).is_some_and(|(a, d)| in_use(&a, std::path::Path::new(p), d))).collect()
}

/// Dry run: what would be imported from `path`, and what is in the way. Nothing is changed.
#[tauri::command]
async fn scan_folder(path: String) -> Vec<Found> {
    found(import::scan(std::path::Path::new(&path)).into_iter().map(|(p, a)| (p, a, false)).collect())
}

/// Looks in the usual places for profiles that are signed in on this computer.
#[tauri::command]
async fn scan_computer() -> Vec<Found> {
    // A stalled drive can block one directory read for minutes, so the walk runs on its own thread and we stop
    // waiting after 20 s whatever it is doing.
    let out: std::sync::Arc<std::sync::Mutex<Vec<import::Hit>>> = Default::default();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (o, d) = (out.clone(), done.clone());
    std::thread::spawn(move || {
        import::scan_computer(signin::default_slot_dir(), &store::data_dir(), &o);
        d.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(14);
        while !done.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    })
    .await;
    let hits = out.lock().unwrap_or_else(|e| e.into_inner()).clone();
    found(hits)
}

/// Copy one found profile in. The copy lands under a temporary name and is renamed only on success.
#[tauri::command]
async fn import_profile(path: String, app: String, name: String, tint: String) -> Res<()> {
    let a = app_of(&app)?;
    let src = PathBuf::from(&path);
    if import::kind_of(&src) != Some(a.key) {
        return Err("That folder isn't a profile for this app.".into());
    }
    if src.starts_with(store::data_dir()) {
        return Err("That profile is already in Tandem.".into());
    }
    if in_use(&a, &src, &engine::user_data_dirs(&a)) {
        return Err("It's open right now. Close it, then try again.".into());
    }
    let _g = store::lock();
    let mut meta = store::load_meta();
    let name = store::validate_name(&meta, a.key, &name, None)?;
    if !store::valid_tint(&tint) {
        return Err("Unknown colour.".into());
    }
    let root = profiles_dir(&a);
    let (dst, tmp) = (root.join(&name), root.join(format!(".importing-{name}")));
    if dst.exists() {
        return Err("A folder with this name already exists.".into());
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = import::copy_profile(&src, &tmp).and_then(|_| std::fs::rename(&tmp, &dst).map_err(|e| e.to_string())) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    meta.profiles.push(Meta { app: a.key.into(), folder: name.clone(), name, tint, last_opened: 0 });
    store::save_meta(&meta)
}

#[tauri::command]
fn usage_snapshot() -> Vec<usage::Usage> {
    usage::snapshot()
}

#[tauri::command]
async fn refresh_usage(handle: tauri::AppHandle) -> Vec<usage::Usage> {
    use tauri_plugin_notification::NotificationExt;
    let all = usage::refresh().await;
    if store::load_settings().warn_near_limit {
        let running: std::collections::HashSet<(String, String)> = [engine::CLAUDE, engine::CODEX]
            .iter()
            .flat_map(|a| engine::running(a, &profiles_dir(a)).profiles.into_keys().map(move |f| (a.key.to_string(), f.to_lowercase())))
            .collect();
        let folders: std::collections::HashMap<(String, String), String> =
            store::load_meta().profiles.into_iter().map(|m| ((m.app, m.name), m.folder.to_lowercase())).collect();
        let is_running = |u: &usage::Usage| folders.get(&(u.app.clone(), u.name.clone())).is_some_and(|f| running.contains(&(u.app.clone(), f.clone())));
        for a in usage::alerts_now(is_running) {
            let _ = handle.notification().builder().title(a.title).body("Open Tandem to see which account has the most room.").show();
        }
    }
    all
}

#[tauri::command]
fn set_settings(handle: tauri::AppHandle, settings: Settings) -> Res<()> {
    store::save_settings(&settings)?;
    apply_settings(&handle, &settings);
    Ok(())
}

fn apply_settings(handle: &tauri::AppHandle, s: &Settings) {
    use tauri_plugin_autostart::ManagerExt;
    tray::set_visible(handle, s.show_tray);
    let auto = handle.autolaunch();
    let _ = if s.open_at_login { auto.enable() } else { auto.disable() };
}

/// `--launch <app> <profile>` from a desktop shortcut.
fn launch_from_args(args: &[String]) -> bool {
    let Some(i) = args.iter().position(|a| a == "--launch") else { return false };
    if let (Some(app), Some(name)) = (args.get(i + 1), args.get(i + 2)) {
        let _ = launch_profile(app, Some(name.clone()));
    }
    true
}

#[tauri::command]
fn get_groups() -> Vec<store::Group> {
    store::load_groups().groups
}

/// Create (`old` None) or edit a group. Members that no longer exist are dropped.
#[tauri::command]
fn save_group(old: Option<String>, name: String, members: Vec<store::Member>) -> Res<()> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Enter a name of 40 characters or fewer.".into());
    }
    let _g = store::lock();
    let have: Vec<(String, String)> = store::load_meta().profiles.into_iter().map(|m| (m.app, m.name)).collect();
    let members: Vec<store::Member> = members.into_iter().filter(|m| have.contains(&(m.app.clone(), m.name.clone()))).collect();
    let mut g = store::load_groups();
    if g.groups.iter().any(|x| x.name.eq_ignore_ascii_case(&name) && Some(&x.name) != old.as_ref()) {
        return Err("You already have a group with this name.".into());
    }
    match old.and_then(|o| g.groups.iter().position(|x| x.name == o)) {
        Some(i) => g.groups[i] = store::Group { name, members },
        None => g.groups.push(store::Group { name, members }),
    }
    store::save_groups(&g)
}

#[tauri::command]
fn delete_group(name: String) -> Res<()> {
    let _g = store::lock();
    let mut g = store::load_groups();
    g.groups.retain(|x| x.name != name);
    store::save_groups(&g)
}

#[tauri::command]
fn add_to_group(group: String, app: String, name: String) -> Res<()> {
    let _g = store::lock();
    let mut g = store::load_groups();
    let grp = g.groups.iter_mut().find(|x| x.name == group).ok_or("Group not found")?;
    let m = store::Member { app, name };
    if !grp.members.contains(&m) {
        grp.members.push(m);
    }
    store::save_groups(&g)
}

/// Opens every member that isn't already running; returns how many were started.
#[tauri::command]
async fn launch_group(name: String) -> Res<usize> {
    let g = store::load_groups().groups.into_iter().find(|x| x.name == name).ok_or("Group not found")?;
    let meta = store::load_meta();
    let mut started = 0;
    for m in g.members {
        let Some(a) = app_by_key(&m.app) else { continue };
        let Some(p) = meta.profiles.iter().find(|p| p.app == m.app && p.name == m.name) else { continue };
        if engine::running(&a, &profiles_dir(&a)).profiles.keys().any(|k| k.eq_ignore_ascii_case(&p.folder)) {
            continue;
        }
        launch_profile(&m.app, Some(m.name))?;
        started += 1;
    }
    Ok(started)
}

#[tauri::command]
async fn set_shortcut(app: String, name: String, on: bool) -> Res<()> {
    let a = app_of(&app)?;
    if on {
        shortcut::create(&a, &name)
    } else {
        shortcut::remove(&a, &name);
        Ok(())
    }
}

/// "Minimize to tray": hide the window when the tray icon exists, otherwise minimize it.
#[tauri::command]
fn hide_main(handle: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = handle.get_webview_window("main") {
        let _ = if store::load_settings().show_tray { w.hide() } else { w.minimize() };
    }
}

#[tauri::command]
fn open_main(handle: tauri::AppHandle, route: Option<String>) {
    tray::show_main(&handle, route.as_deref());
}

#[tauri::command]
fn set_tray_icon(handle: tauri::AppHandle, dark: bool) {
    tray::set_icon(&handle, dark);
}

fn check_route(route: &str, terminal: &str) -> Res<()> {
    match route {
        "web" => Ok(()),
        "terminal" if store::load_meta().profiles.iter().any(|m| m.app == "cc" && m.name == terminal) => Ok(()),
        "terminal" => Err("Pick a Claude Code profile.".into()),
        _ => Err("Unknown route.".into()),
    }
}

/// Saves a profile's session start schedule.
#[tauri::command]
fn save_schedule(app: String, name: String, enabled: bool, time: String, repeat: String, message: String, route: String, terminal: String) -> Res<()> {
    check_route(&route, &terminal)?;
    if app != "claude" || !store::load_meta().profiles.iter().any(|m| m.app == app && m.name == name) {
        return Err("Profile not found".into());
    }
    if !schedule::valid_time(&time) {
        return Err("Enter a time like 08:00.".into());
    }
    let message = message.trim().to_string();
    if message.is_empty() || message.chars().count() > 200 {
        return Err("Enter a message of 200 characters or fewer.".into());
    }
    if repeat != "daily" && repeat != "once" {
        return Err("Unknown repeat option.".into());
    }
    let _g = store::lock();
    let mut f = store::load_schedules();
    match f.schedules.iter_mut().find(|s| s.app == app && s.name == name) {
        Some(s) => {
            // A changed time or a re-enable may fire again today.
            if s.time != time || (enabled && !s.enabled) {
                s.last_day.clear();
            }
            (s.enabled, s.time, s.repeat, s.message, s.route, s.terminal) = (enabled, time, repeat, message, route, terminal);
        }
        None => f.schedules.push(store::Schedule { app, name, enabled, time, repeat, message, route, terminal, ..Default::default() }),
    }
    store::save_schedules(&f)
}

/// BETA. Sends the schedule's message right now; the user asked for it by pressing the button.
#[tauri::command]
async fn send_session_now(app: String, name: String, message: String, route: String, terminal: String) -> Res<()> {
    check_route(&route, &terminal)?;
    schedule::send_and_record(&app, &name, message.trim(), &route, &terminal).await
}

#[tauri::command]
async fn history_refresh() -> Res<()> {
    tauri::async_runtime::spawn_blocking(history::refresh).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn history_status() -> history::Status {
    history::status()
}

#[tauri::command]
async fn history_list(query: String, filter: String) -> Res<Vec<history::SessionRow>> {
    tauri::async_runtime::spawn_blocking(move || history::list(&query, &filter)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn history_session(id: String) -> Res<history::Detail> {
    tauri::async_runtime::spawn_blocking(move || history::detail(&id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn history_import(path: String, app: String, profile: String) -> Res<usize> {
    tauri::async_runtime::spawn_blocking(move || history::import_export(std::path::Path::new(&path), &app, &profile)).await.map_err(|e| e.to_string())?
}

/// (desktop profiles with an imported export, desktop profiles in total)
#[tauri::command]
fn history_coverage() -> (usize, usize) {
    history::export_coverage()
}

#[tauri::command]
async fn history_resume(id: String) -> Res<()> {
    history::resume(&id, &store::load_settings())
}

#[tauri::command]
fn hide_flyout(handle: tauri::AppHandle) {
    tray::hide_flyout(&handle);
}

#[tauri::command]
fn check_name(app: String, name: String, current: Option<String>) -> Option<String> {
    store::validate_name(&store::load_meta(), &app, &name, current.as_deref()).err()
}

#[tauri::command]
fn signin_start(app: String, name: String) -> Res<()> {
    let a = app_of(&app)?;
    if a.is_terminal() {
        return Err("Use the terminal sign-in for this profile.".into());
    }
    let meta = store::load_meta();
    let m = meta.profiles.iter().find(|m| m.app == a.key && m.name == name).ok_or("Profile not found")?;
    let root = profiles_dir(&a);
    signin::start(a, name, root.join(&m.folder), root)
}

/// Opens a terminal that runs the tool's sign-in. The page notices the credential file appearing.
#[tauri::command]
async fn terminal_signin(app: String, name: String) -> Res<()> {
    let a = app_of(&app)?;
    if !a.is_terminal() {
        return Err("Not a terminal profile.".into());
    }
    let meta = store::load_meta();
    let m = meta.profiles.iter().find(|m| m.app == a.key && m.name == name).ok_or("Profile not found")?;
    terminal::open(&a, &m.name, &profiles_dir(&a).join(&m.folder), &store::load_settings(), true)
}

#[tauri::command]
fn signin_status() -> signin::Status {
    signin::status()
}

#[tauri::command]
fn signin_cancel() {
    signin::cancel()
}

/// True when a previous sign-in set the Main instance's session aside and never put it back.
#[tauri::command]
fn signin_orphan() -> bool {
    signin::has_orphan_backup()
}

#[tauri::command]
fn signin_recover(restore: bool) -> Res<()> {
    if restore { signin::restore() } else { signin::discard_backup() }
}

#[tauri::command]
fn quit(handle: tauri::AppHandle) {
    handle.exit(0);
}

pub fn run() {
    use tauri::{Manager, WindowEvent};
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    tauri::Builder::default()
        // A second launch brings the window forward, or just starts the profile when it came from a desktop shortcut.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !launch_from_args(&args) {
                tray::show_main(app, None);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::Pending::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, ev| {
                    if ev.state() == ShortcutState::Pressed {
                        tray::toggle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let h = app.handle();
            tray::setup(h)?;
            #[cfg(target_os = "macos")]
            tray::app_menu(h)?;
            schedule::start(h.clone());
            let settings = store::load_settings();
            apply_settings(h, &settings);
            // shortcut: fixed default, no "Change" yet; another app may already own it, so a failure is ignored.
            let _ = h.global_shortcut().register(if cfg!(target_os = "macos") { "Alt+Super+Space" } else { "CommandOrControl+Shift+Space" });
            let args: Vec<String> = std::env::args().collect();
            let shortcut_launch = launch_from_args(&args);
            let hidden = (shortcut_launch || args.iter().any(|a| a == "--hidden")) && settings.show_tray;
            if shortcut_launch && !settings.show_tray {
                // Nothing could bring the window back, so quit once the profile has started.
                let hh = h.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    hh.exit(0);
                });
            } else if !hidden {
                if let Some(w) = h.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if w.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = e {
                if store::load_settings().show_tray {
                    api.prevent_close();
                    let _ = w.hide();
                } else {
                    w.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state, create_profile, update_profile, delete_profile, launch, close, show_folder, set_settings, check_name, scan_folder, scan_computer, recheck_in_use, import_profile, signin_start, signin_status, signin_cancel,
            signin_orphan, signin_recover, usage_snapshot, refresh_usage, updater::check_update, updater::install_update, quit, get_groups, save_group, delete_group, add_to_group, launch_group, set_shortcut, hide_main, open_main, hide_flyout, set_tray_icon, save_schedule, send_session_now, terminal_signin, history_refresh, history_status, history_list, history_session, history_import, history_coverage, history_resume
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tandem")
        .run(|_app, _event| {
            // Clicking the Dock icon while the window is hidden in the menu bar brings it back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = _event {
                tray::show_main(_app, None);
            }
        });
}

/// Lower-case, backslashes, no trailing slash: how --user-data-dir values are compared.
fn norm(p: &std::path::Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\").trim_end_matches('\\').to_string()
}

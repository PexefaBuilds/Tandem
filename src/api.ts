import { invoke } from "@tauri-apps/api/core";

export type AppKey = "claude" | "codex" | "cc" | "cx";
export type Tint = "graphite" | "blue" | "green" | "amber" | "rose" | "teal";
export const TINTS: Tint[] = ["graphite", "blue", "green", "amber", "rose", "teal"];
export type AfterLaunch = "minimize" | "hide" | "tray" | "stay" | "quit";

export interface AppInfo { key: AppKey; name: string; installed: boolean; version: string; path: string; mainRunning: boolean }
export interface Profile { app: AppKey; name: string; tint: Tint; folder: string; running: boolean; signedIn: boolean; hasShortcut: boolean; lastOpened: number; account: string }
export interface SignInStatus { phase: "idle" | "waiting" | "detected" | "ready" | "failed" | "cancelled"; message: string }
export interface Settings { afterLaunch: AfterLaunch; welcomeDone: boolean; refreshMinutes: number; warnNearLimit: boolean; showTray: boolean; openAtLogin: boolean; terminal: "wt" | "powershell" | "iterm" | "ghostty"; keepSeparate: boolean }
export interface UsageRow { label: string; pct: number; resetsAt: string }
/** kind: "" (fresh) | "expired" | "locked" | "offline" | "limited" | "blocked" | "other". Old rows are kept on failure. */
export interface UpdateInfo { current: string; latest: string; newer: boolean }
export interface Usage { app: AppKey; name: string; kind: string; message: string; account: string; plan: string; rows: UsageRow[]; updated: number }
export interface Found { path: string; name: string; app: AppKey; hasSignIn: boolean; inUse: boolean; isDefault: boolean }
export interface Schedule { app: AppKey; name: string; enabled: boolean; time: string; repeat: string; message: string; route: string; terminal: string; lastSent: number; lastStatus: string; lastNote: string; lastDay: string }
export interface Member { app: AppKey; name: string }
export interface Group { name: string; members: Member[] }
export interface HistoryRow { id: string; kind: "cc" | "cx" | "claude" | "chatgpt"; profile: string; title: string; cwd: string; updated: number; count: number; snippet: string }
export interface HistoryDetail { row: HistoryRow; messages: { role: "user" | "assistant"; text: string }[]; shownAll: boolean }
export interface State { apps: AppInfo[]; profiles: Profile[]; settings: Settings; dataDir: string; schedules: Schedule[] }

export const api = {
  state: () => invoke<State>("get_state"),
  create: (app: AppKey, name: string, tint: Tint) => invoke<void>("create_profile", { app, name, tint }),
  update: (app: AppKey, name: string, newName: string, tint: Tint) => invoke<void>("update_profile", { app, name, newName, tint }),
  remove: (app: AppKey, name: string) => invoke<void>("delete_profile", { app, name }),
  launch: (app: AppKey, name: string | null) => invoke<void>("launch", { app, name }),
  close: (app: AppKey, name: string | null) => invoke<number>("close", { app, name }),
  showFolder: (app: AppKey, name: string | null) => invoke<void>("show_folder", { app, name }),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  checkName: (app: AppKey, name: string, current: string | null) => invoke<string | null>("check_name", { app, name, current }),
  scanFolder: (path: string) => invoke<Found[]>("scan_folder", { path }),
  scanComputer: () => invoke<Found[]>("scan_computer"),
  recheckInUse: (items: [string, AppKey][]) => invoke<boolean[]>("recheck_in_use", { items }),
  importProfile: (path: string, app: AppKey, name: string, tint: Tint) => invoke<void>("import_profile", { path, app, name, tint }),
  signinStart: (app: AppKey, name: string) => invoke<void>("signin_start", { app, name }),
  signinStatus: () => invoke<SignInStatus>("signin_status"),
  signinCancel: () => invoke<void>("signin_cancel"),
  signinOrphan: () => invoke<boolean>("signin_orphan"),
  signinRecover: (restore: boolean) => invoke<void>("signin_recover", { restore }),
  usageSnapshot: () => invoke<Usage[]>("usage_snapshot"),
  refreshUsage: () => invoke<Usage[]>("refresh_usage"),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
  quit: () => invoke<void>("quit"),
  groups: () => invoke<Group[]>("get_groups"),
  saveGroup: (old: string | null, name: string, members: Member[]) => invoke<void>("save_group", { old, name, members }),
  deleteGroup: (name: string) => invoke<void>("delete_group", { name }),
  addToGroup: (group: string, app: AppKey, name: string) => invoke<void>("add_to_group", { group, app, name }),
  launchGroup: (name: string) => invoke<number>("launch_group", { name }),
  setShortcut: (app: AppKey, name: string, on: boolean) => invoke<void>("set_shortcut", { app, name, on }),
  hideMain: () => invoke<void>("hide_main"),
  openMain: (route: string | null) => invoke<void>("open_main", { route }),
  setTrayIcon: (dark: boolean) => invoke<void>("set_tray_icon", { dark }),
  saveSchedule: (app: AppKey, name: string, enabled: boolean, time: string, repeat: string, message: string, route: string, terminal: string) => invoke<void>("save_schedule", { app, name, enabled, time, repeat, message, route, terminal }),
  sendSessionNow: (app: AppKey, name: string, message: string, route: string, terminal: string) => invoke<void>("send_session_now", { app, name, message, route, terminal }),
  terminalSignin: (app: AppKey, name: string) => invoke<void>("terminal_signin", { app, name }),
  historyRefresh: () => invoke<void>("history_refresh"),
  historyStatus: () => invoke<{ indexing: boolean; done: number; total: number }>("history_status"),
  historyList: (query: string, filter: string) => invoke<HistoryRow[]>("history_list", { query, filter }),
  historySession: (id: string) => invoke<HistoryDetail>("history_session", { id }),
  historyImport: (path: string, app: AppKey, profile: string) => invoke<number>("history_import", { path, app, profile }),
  historyCoverage: () => invoke<[number, number]>("history_coverage"),
  historyResume: (id: string) => invoke<void>("history_resume", { id }),
  hideFlyout: () => invoke<void>("hide_flyout"),
};

export const APP_LABEL: Record<AppKey, string> = { claude: "Claude", codex: "Codex", cc: "Claude Code", cx: "Codex CLI" };
export const errText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "Something went wrong.");
export const initial = (name: string) => (name.match(/[\p{L}\p{N}]/u)?.[0] ?? "?").toUpperCase();

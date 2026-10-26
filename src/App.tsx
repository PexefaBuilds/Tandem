import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { api, errText, type AppKey, type Group, type Profile, type State, type Usage } from "./api";
import { Sidebar, type Route } from "./Sidebar";
import { TopBar } from "./TopBar";
import { Profiles, type Ctx } from "./Profiles";
import { Settings } from "./Settings";
import { DeleteProfile, EditProfile, NewProfile } from "./Sheet";
import { isMac } from "./platform";
import { isDark } from "./theme";
import { ImportSheet } from "./Import";
import { Welcome } from "./Welcome";
import { Groups } from "./Groups";
import { SessionStart } from "./SessionStart";
import { History } from "./History";
import { TerminalSignIn, Terminals } from "./Terminals";
import { Recover, SignIn } from "./SignIn";

type SheetState = { kind: "signin"; p: Profile } | { kind: "recover" } | { kind: "import" } | { kind: "new"; app: AppKey } | { kind: "edit"; p: Profile } | { kind: "delete"; p: Profile } | { kind: "session"; p: Profile } | { kind: "tsignin"; p: Profile } | null;

const installed = (s: State) => Object.fromEntries(s.apps.map((a) => [a.key, a.installed])) as Record<AppKey, boolean>;

export default function App() {
  const [state, setState] = useState<State | null>(null);
  const [error, setError] = useState("");
  const [route, setRoute] = useState<Route>("claude");
  const [query, setQuery] = useState("");
  const [groups, setGroups] = useState<Group[]>([]);
  const [newGroup, setNewGroup] = useState(false);
  const wasRunning = useRef<Set<string>>(new Set());
  const [sheet, setSheet] = useState<SheetState>(null);
  const [toastMsg, setToastMsg] = useState("");
  const toastTimer = useRef<number>(undefined);
  const searchRef = useRef<HTMLInputElement>(null);
  const [usage, setUsage] = useState<Usage[]>([]);
  const [busy, setBusy] = useState(false);
  const refreshUsage = useCallback(() => {
    setBusy(true);
    return api.refreshUsage().then(setUsage).catch((e) => setError(errText(e))).finally(() => setBusy(false));
  }, []);
  const minutes = state?.settings.refreshMinutes;
  useEffect(() => {
    if (minutes === undefined) return;
    api.usageSnapshot().then(setUsage).catch(() => {});
    const first = window.setTimeout(refreshUsage, 0);
    const t = minutes ? window.setInterval(refreshUsage, minutes * 60000) : undefined;
    return () => { window.clearTimeout(first); window.clearInterval(t); };
  }, [minutes, refreshUsage]);

  const reload = useCallback(() => {
    api.state().then((s) => {
      setState(s);
      setError("");
      // A profile that just closed has released its cookie file, so read its usage now.
      const now = new Set(s.profiles.filter((p) => p.running).map((p) => p.app + "/" + p.name));
      if ([...wasRunning.current].some((k) => !now.has(k))) refreshUsage();
      wasRunning.current = now;
    }).catch((e) => setError(errText(e)));
    api.groups().then(setGroups).catch(() => {});
  }, [refreshUsage]);
  useEffect(() => {
    reload();
    const t = window.setInterval(reload, 3000); // running status
    return () => window.clearInterval(t);
  }, [reload]);

  useEffect(() => {
    const un = listen<string>("navigate", (e) => setRoute(e.payload as Route));
    return () => { un.then((f) => f()); };
  }, []);

  useEffect(() => { api.signinOrphan().then((o) => o && setSheet({ kind: "recover" })).catch(() => {}); }, []);

  const toast = useCallback((m: string) => {
    setToastMsg(m);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToastMsg(""), 2600);
  }, []);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") { e.preventDefault(); searchRef.current?.focus(); searchRef.current?.select(); }
      else if (e.key === "Escape") {
        if (sheet) setSheet(null);
        else if (document.activeElement === searchRef.current) { setQuery(""); searchRef.current?.blur(); }
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [sheet]);

  const ctx: Ctx = {
    reload,
    toast,
    edit: (p) => setSheet({ kind: "edit", p }),
    del: (p) => setSheet({ kind: "delete", p }),
    newProfile: (app) => setSheet({ kind: "new", app }),
    importProfiles: () => setSheet({ kind: "import" }),
    signin: (p) => setSheet({ kind: p.app === "cc" || p.app === "cx" ? "tsignin" : "signin", p }),
    groups,
    schedules: state?.schedules ?? [],
    sessionStart: (p) => setSheet({ kind: "session", p }),
    newGroup: () => { setNewGroup(true); setRoute("groups"); },
    afterLaunch: () => {
      const mode = state?.settings.afterLaunch;
      if (mode === "quit") api.quit();
      else if (mode === "hide") api.hideMain();
      else if (mode === "minimize" || mode === "tray") getCurrentWindow().minimize(); // "tray" is the old name for minimize
    },
  };
  const finishWelcome = (n: number) => {
    api.setSettings({ ...state!.settings, welcomeDone: true }).then(() => { reload(); if (n) toast(`Brought over ${n} ${n === 1 ? "profile" : "profiles"}`); }).catch((e) => toast(errText(e)));
  };
  const done = (m: string) => { setSheet(null); toast(m); reload(); };

  const appInfo = state?.apps.find((a) => a.key === route);
  return (
    <div style={{ display: "flex", height: "100vh", background: "var(--ground)" }}>
      <div style={{ position: "absolute", top: 8, left: "50%", transform: "translateX(-50%)", width: 440, height: 32, zIndex: 5, display: "flex", alignItems: "center" }}>
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" style={{ position: "absolute", left: 13, color: "var(--faint)" }} aria-hidden="true"><circle cx="7" cy="7" r="4.5" /><path d="m10.5 10.5 3 3" strokeLinecap="round" /></svg>
        <label htmlFor="search" className="sr-only">Search profiles</label>
        <input ref={searchRef} id="search" className="field" value={query} onChange={(e) => setQuery(e.target.value)} style={{ height: 32, paddingLeft: 36, paddingRight: 64, borderColor: "var(--border)", borderRadius: 999, fontSize: 13 }} placeholder="Search profiles and groups" />
        <span style={{ position: "absolute", right: 14, fontSize: 12, color: "var(--faint)" }}>{isMac ? "⌘ K" : "Ctrl K"}</span>
      </div>
      {!state && (
        <div role="status" className="rise" style={{ position: "absolute", inset: 0, zIndex: 50, background: "var(--ground)", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 20 }}>
          <img src={isDark() ? "/logo-dark.svg" : "/logo.svg"} width="72" height="72" alt="" />
          <div style={{ fontSize: 14, color: "var(--muted)" }}>Gathering your profiles…</div>
          <div className="wait" style={{ width: 200 }} />
        </div>
      )}
      <Sidebar route={route} onNav={(r) => { setRoute(r); setQuery(""); }} state={state} />
      <div style={{ flex: "1 1 auto", minWidth: 0, display: "flex", flexDirection: "column", padding: "0 8px 8px 0" }}>
        <TopBar busy={busy} updated={Math.max(0, ...usage.map((u) => u.updated))} onRefresh={() => { reload(); refreshUsage().then(() => toast("Usage refreshed")); }} />
        <main className="sheet">
          {error && <div role="alert" style={{ marginBottom: 16, padding: "12px 16px", borderRadius: 12, background: "var(--raised)", color: "var(--negative)", fontSize: 14 }}>{error}</div>}
          {state && appInfo && (route === "claude" || route === "codex") && (
            <Profiles key={route} app={appInfo} profiles={state.profiles.filter((p) => p.app === route)} usage={usage.filter((u) => u.app === route)} retry={refreshUsage} query={query} ctx={ctx} dataDir={state.dataDir} />
          )}
          {state && route === "settings" && <Settings state={state} reload={reload} toast={toast} />}
          {state && route === "groups" && <Groups state={state} groups={groups} reload={reload} toast={toast} afterLaunch={ctx.afterLaunch} creating={newGroup} onCreated={() => setNewGroup(false)} />}
          {state && appInfo && (route === "cc" || route === "cx") && <Terminals key={route} app={appInfo} state={state} ctx={ctx} />}
          {state && route === "history" && <History state={state} toast={toast} reload={reload} />}
          {toastMsg && (
            <div role="status" className="rise" style={{ position: "sticky", bottom: 0, display: "flex", justifyContent: "center", pointerEvents: "none", marginTop: 16 }}>
              <span style={{ padding: "0 16px", height: 36, display: "inline-flex", alignItems: "center", borderRadius: 999, background: "var(--primary-bg)", color: "var(--primary-ink)", fontSize: 13, fontWeight: 500 }}>{toastMsg}</span>
            </div>
          )}
        </main>
      </div>
      {sheet?.kind === "recover" && <Recover onDone={done} />}
      {sheet?.kind === "signin" && <SignIn p={sheet.p} onClose={() => setSheet(null)} onDone={done} onLaunch={(p) => api.launch(p.app, p.name).then(reload).catch((e) => toast(errText(e)))} />}
      {sheet?.kind === "session" && state && <SessionStart p={sheet.p} schedule={state.schedules.find((s) => s.app === sheet.p.app && s.name === sheet.p.name)} onClose={() => setSheet(null)} onDone={done} />}
      {sheet?.kind === "tsignin" && state && <TerminalSignIn p={sheet.p} live={state.profiles.find((x) => x.app === sheet.p.app && x.name === sheet.p.name)} onClose={() => setSheet(null)} onDone={done} />}
      {sheet?.kind === "import" && state && <ImportSheet installed={installed(state)} onClose={() => setSheet(null)} onDone={done} />}
      {state && !state.settings.welcomeDone && <Welcome state={state} onFinish={finishWelcome} />}
      {sheet?.kind === "new" && state && <NewProfile app={sheet.app} apps={state.apps.map((a) => ({ key: a.key, name: a.name }))} onClose={() => setSheet(null)} onDone={done} onSignIn={(app, name) => api.state().then((s) => { setState(s); const p = s.profiles.find((x) => x.app === app && x.name === name); if (p) setSheet({ kind: p.app === "cc" || p.app === "cx" ? "tsignin" : "signin", p }); })} />}
      {sheet?.kind === "edit" && <EditProfile p={sheet.p} onClose={() => setSheet(null)} onDone={done} />}
      {sheet?.kind === "delete" && <DeleteProfile p={sheet.p} onClose={() => setSheet(null)} onDone={done} />}
    </div>
  );
}

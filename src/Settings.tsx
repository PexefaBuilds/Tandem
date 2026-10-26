import { api, errText, type AfterLaunch, type State } from "./api";
import { applyTheme, savedTheme, type Theme } from "./theme";
import { useState } from "react";
import { terminalChoices } from "./platform";
import { openUrl } from "@tauri-apps/plugin-opener";

function Seg<T extends string>({ value, options, onChange, label }: { value: T; options: [T, string][]; onChange: (v: T) => void; label: string }) {
  return (
    <div className="seg" role="radiogroup" aria-label={label}>
      {options.map(([v, t]) => <button key={v} role="radio" aria-checked={value === v} className={value === v ? "on" : ""} onClick={() => onChange(v)}>{t}</button>)}
    </div>
  );
}

const Row = ({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) => (
  <div className="set-row">
    <div><div style={{ fontSize: 14, fontWeight: 500 }}>{title}</div>{hint && <div style={{ fontSize: 13, color: "var(--muted)", marginTop: 2 }}>{hint}</div>}</div>
    <div>{children}</div>
  </div>
);

export function Settings({ state, reload, toast }: { state: State; reload: () => void; toast: (m: string) => void }) {
  const [theme, setTheme] = useState<Theme>(savedTheme());
  const setAfter = (afterLaunch: AfterLaunch) => api.setSettings({ ...state.settings, afterLaunch }).then(reload).catch((e) => toast(errText(e)));
  const [upd, setUpd] = useState<{ text: string; url?: string } | null>(null);
  const [checking, setChecking] = useState(false);
  const checkUpdate = () => {
    setChecking(true);
    api.checkUpdate()
      .then((u) => setUpd(u.newer ? { text: `Version ${u.latest.replace(/^v/, "")} is available.`, url: u.url } : { text: `You're up to date (${u.current}).` }))
      .catch((e) => setUpd({ text: errText(e) }))
      .finally(() => setChecking(false));
  };
  const save = (patch: Partial<State["settings"]>) => api.setSettings({ ...state.settings, ...patch }).then(reload).catch((e) => toast(errText(e)));
  return (
    <div style={{ maxWidth: 820, margin: "0 auto" }}>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 600, letterSpacing: "-0.015em" }}>Settings</h1>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>General</h2>
      <Row title="After launching a profile" hint="Minimize keeps Tandem in the taskbar and the tray. Hide to tray removes it from the taskbar until you open it from the tray icon.">
        <Seg label="After launching a profile" value={state.settings.afterLaunch === "tray" ? "minimize" : state.settings.afterLaunch} onChange={setAfter} options={[["minimize", "Minimize"], ["hide", "Hide to tray"], ["stay", "Stay open"], ["quit", "Quit"]]} />
      </Row>
      <Row title="Show in the system tray" hint="A small icon near the clock that opens a quick launcher. Closing the window keeps Tandem running there.">
        <button className={state.settings.showTray ? "switch on" : "switch"} role="switch" aria-checked={state.settings.showTray} aria-label="Show in the system tray" onClick={() => save({ showTray: !state.settings.showTray })} />
      </Row>
      <Row title="Open at login" hint="Starts Tandem hidden in the tray when you sign in to your computer">
        <button className={state.settings.openAtLogin ? "switch on" : "switch"} role="switch" aria-checked={state.settings.openAtLogin} aria-label="Open at login" onClick={() => save({ openAtLogin: !state.settings.openAtLogin })} />
      </Row>
      <Row title="Quick launch shortcut" hint="Opens the tray launcher from anywhere">
        <span className="mono" style={{ fontSize: 13 }}>{navigator.userAgent.includes("Mac") ? "⌥⌘Space" : "Ctrl+Shift+Space"}</span>
      </Row>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>Usage limits</h2>
      <Row title="Refresh usage" hint="How often limits are checked for every signed-in profile">
        <Seg label="Refresh usage" value={String(state.settings.refreshMinutes)} onChange={(v) => save({ refreshMinutes: Number(v) })} options={[["1", "1 min"], ["5", "5 min"], ["15", "15 min"], ["0", "Manual"]]} />
      </Row>
      <Row title="Warn me near a limit" hint="A notification when a running profile passes 85% of its 5-hour limit or 90% of its weekly limit">
        <button className={state.settings.warnNearLimit ? "switch on" : "switch"} role="switch" aria-checked={state.settings.warnNearLimit} aria-label="Warn me near a limit" onClick={() => save({ warnNearLimit: !state.settings.warnNearLimit })} />
      </Row>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>Terminal profiles</h2>
      <Row title="Open terminal profiles in" hint="Used by Claude Code and Codex CLI profiles">
        <Seg label="Terminal" value={state.settings.terminal} onChange={(terminal) => save({ terminal })} options={terminalChoices} />
      </Row>
      <Row title="Keep accounts separate" hint="Ignore API keys set on this computer, so each profile uses its own sign-in">
        <button className={state.settings.keepSeparate ? "switch on" : "switch"} role="switch" aria-checked={state.settings.keepSeparate} aria-label="Keep accounts separate" onClick={() => save({ keepSeparate: !state.settings.keepSeparate })} />
      </Row>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>Appearance</h2>
      <Row title="Theme">
        <Seg label="Theme" value={theme} onChange={(t) => { applyTheme(t); setTheme(t); }} options={[["system", "System"], ["light", "Light"], ["dark", "Dark"]]} />
      </Row>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>Apps and storage</h2>
      {state.apps.map((a) => (
        <Row key={a.key} title={a.name}>
          <div style={{ fontSize: 13, color: a.installed ? "var(--text)" : "var(--muted)" }}>{a.installed ? `Version ${a.version}` : "Not installed"}</div>
          {a.installed && <div className="mono" style={{ fontSize: 12, color: "var(--muted)", marginTop: 4, wordBreak: "break-all" }}>{a.path}</div>}
        </Row>
      ))}
      <Row title="Profiles folder">
        <div className="mono" style={{ fontSize: 12, color: "var(--muted)", wordBreak: "break-all" }}>{state.dataDir}</div>
        <button className="btn" style={{ marginTop: 10 }} onClick={() => api.showFolder("claude", null).catch((e) => toast(errText(e)))}>Show folder</button>
      </Row>
      <h2 style={{ margin: "24px 0 8px", fontSize: 16, fontWeight: 600 }}>About</h2>
      <Row title="Tandem" hint="Your Claude and Codex accounts, in tandem.">
        <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <button className="btn" disabled={checking} onClick={checkUpdate}>{checking ? "Checking…" : "Check for updates"}</button>
          {upd && <span role="status" style={{ fontSize: 13, color: "var(--muted)" }}>{upd.text}</span>}
          {upd?.url && <button className="btn primary" onClick={() => openUrl(upd.url!).catch((e) => toast(errText(e)))}>Download</button>}
        </div>
      </Row>
    </div>
  );
}

import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errText, initial, type AppInfo, type Profile, type State } from "./api";
import { Menu, type Ctx, type Row } from "./Profiles";
import { Sheet } from "./Sheet";
import { terminalChoices, type Terminal } from "./platform";

const CFG: Record<"cc" | "cx", { env: string; cmd: string; login: string }> = {
  cc: { env: "CLAUDE_CONFIG_DIR", cmd: "claude", login: "Type /login in the terminal that opens, then follow the browser prompt." },
  cx: { env: "CODEX_HOME", cmd: "codex", login: "The terminal that opens runs “codex login”. Finish signing in in your browser." },
};

const short = (p: string) => (p.length > 38 ? "…" + p.slice(-37) : p);
const ago = (t: number) => {
  if (!t) return "Never";
  const m = Math.floor((Date.now() / 1000 - t) / 60);
  return m < 1 ? "Just now" : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.floor(m / 60)} h ago` : `${Math.floor(m / 1440)} d ago`;
};

/** Opens the tool's sign-in in a terminal, then closes itself once the credential file shows up. */
export function TerminalSignIn({ p, live, onClose, onDone }: { p: Profile; live: Profile | undefined; onClose: () => void; onDone: (m: string) => void }) {
  const [err, setErr] = useState("");
  const [opened, setOpened] = useState(false);
  const cfg = CFG[p.app as "cc" | "cx"];
  const open = () => api.terminalSignin(p.app, p.name).then(() => { setOpened(true); setErr(""); }).catch((e) => setErr(errText(e)));
  const started = useRef(false);
  useEffect(() => { if (!started.current) { started.current = true; open(); } }, []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (opened && live?.signedIn) onDone(`${p.name} is signed in`); }, [opened, live?.signedIn]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <Sheet title={`Sign in · ${p.name}`} onClose={onClose}>
      <p style={{ margin: "14px 0 0", fontSize: 14, lineHeight: 1.5, color: "var(--muted)" }}>{cfg.login}</p>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 18, fontSize: 13, color: "var(--muted)" }}>
        <span className="spinner" aria-hidden="true" />{opened ? "Waiting for you to finish signing in…" : "Opening a terminal…"}
      </div>
      {err && <div role="alert" style={{ marginTop: 12, fontSize: 13, color: "var(--negative)" }}>{err}</div>}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
        <button className="btn" onClick={open}>Open the terminal again</button>
        <button className="btn primary" onClick={onClose}>Close</button>
      </div>
    </Sheet>
  );
}

export function Terminals({ app, state, ctx }: { app: AppInfo; state: State; ctx: Ctx }) {
  const key = app.key as "cc" | "cx";
  const cfg = CFG[key];
  const rows = state.profiles.filter((p) => p.app === key);
  const [menu, setMenu] = useState<string | null>(null);
  const open = (p: Profile) => api.launch(p.app, p.name).then(ctx.reload).catch((e) => ctx.toast(errText(e)));
  const setTerminal = (terminal: Terminal) => api.setSettings({ ...state.settings, terminal }).then(ctx.reload).catch((e) => ctx.toast(errText(e)));
  return (
    <>
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 16 }}>
        <div>
          <h1 style={{ margin: 0, fontSize: 22, fontWeight: 600, letterSpacing: "-0.015em" }}>{app.name}</h1>
          <div style={{ marginTop: 4, fontSize: 14, color: "var(--muted)" }}>{app.installed ? `Version ${app.version} · ${rows.length} ${rows.length === 1 ? "profile" : "profiles"}` : "Not installed"}</div>
        </div>
        <button className="btn" onClick={() => ctx.newProfile(key)}>New profile</button>
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap", margin: "16px 0 0", padding: "12px 16px", borderRadius: 12, background: "var(--raised)", fontSize: 13 }}>
        <div style={{ color: "var(--muted)" }}>Each profile has its own sign-in. It sets <span className="mono" style={{ fontSize: 13 }}>{cfg.env}</span> to the profile’s folder, then runs <span className="mono" style={{ fontSize: 13 }}>{cfg.cmd}</span>.</div>
        <div style={{ display: "flex", alignItems: "center", gap: 10, color: "var(--muted)" }}>
          Opens in
          <div className="seg" role="radiogroup" aria-label="Terminal">
            {terminalChoices.map(([v, t]) => <button key={v} role="radio" aria-checked={state.settings.terminal === v} className={state.settings.terminal === v ? "on" : ""} onClick={() => setTerminal(v)}>{t}</button>)}
          </div>
        </div>
      </div>
      {!app.installed && (
        <div style={{ margin: "16px 0 0", padding: "12px 16px", borderRadius: 12, border: "1px solid var(--border)", fontSize: 13, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <span style={{ color: "var(--muted)" }}>{app.name} wasn’t found on this PC, so profiles can’t open yet.</span>
          <button className="btn" onClick={() => openUrl(key === "cc" ? "https://docs.claude.com/en/docs/claude-code/setup" : "https://github.com/openai/codex").catch((e) => ctx.toast(errText(e)))}>How to install</button>
        </div>
      )}
      <div className="trow head" style={{ marginTop: 16 }}>
        <span /><span>Profile</span><span>Folder</span><span>Last opened</span><span /><span />
      </div>
      {rows.map((p) => {
        const r: Row = { key: p.name, name: p.name, sub: p.account || (p.signedIn ? "Signed in" : "Signed out"), tint: p.tint, running: false, signedOut: !p.signedIn, profile: p };
        return (
          <div key={p.name} className="trow">
            <span />
            <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0 }}>
              <span className={`av tint-${p.tint}`}>{initial(p.name)}</span>
              <div style={{ display: "flex", flexDirection: "column", minWidth: 0, lineHeight: 1.3 }}>
                <span style={{ fontSize: 14, fontWeight: 500 }}>{p.name}</span>
                <span style={{ fontSize: 12, color: "var(--muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{r.sub}</span>
              </div>
            </div>
            <span className="mono" title={p.folder} style={{ fontSize: 12, color: "var(--muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{short(p.folder)}</span>
            <span style={{ fontSize: 13, color: "var(--muted)" }}>{ago(p.lastOpened)}</span>
            <div style={{ display: "flex", justifyContent: "flex-end" }}>
              {p.signedIn
                ? <button className="btn" disabled={!app.installed} onClick={() => open(p)}>Open terminal</button>
                : <button className="btn" disabled={!app.installed} onClick={() => ctx.signin(p)}>Sign in</button>}
            </div>
            <div style={{ position: "relative" }}>
              <button className="icon-btn" aria-label={`More for ${p.name}`} aria-haspopup="menu" aria-expanded={menu === p.name} onClick={() => setMenu(menu === p.name ? null : p.name)}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><circle cx="3.5" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.5" cy="8" r="1.3" /></svg>
              </button>
              {menu === p.name && <Menu row={r} ctx={ctx} app={key} onClose={() => setMenu(null)} />}
            </div>
          </div>
        );
      })}
      {!rows.length && <div style={{ padding: "40px 0", textAlign: "center", fontSize: 14, color: "var(--muted)" }}>No profiles yet. Create one, then sign in.</div>}
    </>
  );
}

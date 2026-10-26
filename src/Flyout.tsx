import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, APP_LABEL, errText, initial, type Group, type State, type Usage } from "./api";
import { Meter, windows } from "./Profiles";

/** The tray launcher: a small window that opens above the tray icon. */
export function Flyout() {
  const [state, setState] = useState<State | null>(null);
  const [usage, setUsage] = useState<Usage[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [q, setQ] = useState("");
  const [msg, setMsg] = useState("");
  const load = useCallback(() => {
    api.state().then(setState).catch(() => {});
    api.usageSnapshot().then(setUsage).catch(() => {});
    api.groups().then(setGroups).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    const un = listen("flyout-shown", () => { setQ(""); setMsg(""); load(); });
    const esc = (e: KeyboardEvent) => e.key === "Escape" && api.hideFlyout();
    document.addEventListener("keydown", esc);
    const poll = window.setInterval(() => document.visibilityState === "visible" && load(), 3000);
    return () => { window.clearInterval(poll); un.then((f) => f()); document.removeEventListener("keydown", esc); };
  }, [load]);

  const run = (p: Promise<unknown>) => p.then(() => api.hideFlyout()).catch((e) => setMsg(errText(e)));
  const match = (s: string) => s.toLowerCase().includes(q.trim().toLowerCase());
  const profiles = (state?.profiles ?? []).filter((p) => match(p.name));
  const signedIn = (state?.profiles ?? []).filter((p) => p.signedIn).length;
  const shownGroups = groups.filter((g) => match(g.name));

  const label = (t: string) => <div style={{ display: "flex", alignItems: "center", height: 28, padding: "8px 8px 0", fontSize: 12, color: "var(--muted)" }}>{t}</div>;
  const rowStyle: React.CSSProperties = { display: "flex", alignItems: "center", gap: 10, width: "100%", height: 36, padding: "0 8px", border: 0, borderRadius: 6, background: "transparent", color: "var(--text)", fontSize: 13, textAlign: "left", cursor: "pointer" };
  const foot: React.CSSProperties = { flex: 1, height: 32, border: 0, borderRadius: 6, background: "transparent", color: "var(--text)", fontSize: 13, cursor: "pointer" };

  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column", background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 8, overflow: "hidden" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "14px 16px 8px" }}>
        <svg width="22" height="22" viewBox="5 2 38 44" aria-hidden="true"><rect x="7" y="4" width="15" height="32" rx="7.5" fill="var(--claude)" /><rect x="26" y="12" width="15" height="32" rx="7.5" fill="var(--codex)" /></svg>
        <div style={{ lineHeight: 1.15 }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>Tandem</div>
          <div style={{ fontSize: 12, color: "var(--muted)" }}>{signedIn} signed in</div>
        </div>
      </div>
      <div style={{ padding: "0 12px 8px" }}>
        <label htmlFor="fq" className="sr-only">Search profiles</label>
        <input id="fq" className="field" autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search profiles and groups" style={{ height: 32, borderRadius: 999, fontSize: 13 }} />
      </div>
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "0 8px 8px" }}>
        {(["claude", "codex", "cc", "cx"] as const).map((app) => {
          const rows = profiles.filter((p) => p.app === app);
          if (!rows.length) return null;
          return (
            <div key={app}>
              {label(APP_LABEL[app])}
              {rows.map((p) => {
                const u = usage.find((x) => x.app === p.app && x.name === p.name);
                const [short, long] = windows(u);
                const act = () => (p.running ? api.close(p.app, p.name) : api.launch(p.app, p.name)).then(() => { load(); if (!p.running) api.hideFlyout(); }).catch((e) => setMsg(errText(e)));
                return (
                  <div key={p.name} style={{ padding: 10, marginBottom: 6, borderRadius: 8, background: "var(--raised)" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <span className={`av tint-${p.tint}`} style={{ width: 28, height: 28, fontSize: 12 }}>{initial(p.name)}</span>
                      <div style={{ flex: 1, minWidth: 0, lineHeight: 1.3 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, fontWeight: 600 }}>
                          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.name}</span>
                          {p.running && <span aria-label="Running" style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--positive)", flex: "none" }} />}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{[u?.plan, u?.account || p.account].filter(Boolean).join(" · ") || (p.signedIn ? "Signed in" : "Signed out")}</div>
                      </div>
                      <button className={p.running ? "btn" : "btn primary"} style={{ height: 28, padding: "0 12px" }} onClick={act}>{p.running ? "Close" : p.app === "cc" || p.app === "cx" ? "Open" : "Launch"}</button>
                    </div>
                    {short || long ? (
                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginTop: 10 }}>
                        <div><div style={{ fontSize: 11, color: "var(--muted)", marginBottom: 4 }}>5-hour</div><Meter r={short} short="5-hour" /></div>
                        <div><div style={{ fontSize: 11, color: "var(--muted)", marginBottom: 4 }}>Weekly</div><Meter r={long} short="weekly" /></div>
                      </div>
                    ) : u?.message ? <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>{u.message}</div> : null}
                  </div>
                );
              })}
            </div>
          );
        })}
        {!!shownGroups.length && label("Groups")}
        {shownGroups.map((g) => (
          <button key={g.name} style={rowStyle} onClick={() => run(api.launchGroup(g.name))}>
            <span style={{ flex: 1 }}>{g.name}</span>
            <span style={{ fontSize: 12, color: "var(--muted)" }}>Launch all {g.members.length}</span>
          </button>
        ))}
        {state && !profiles.length && !shownGroups.length && <div style={{ padding: "40px 0", textAlign: "center", fontSize: 13, color: "var(--muted)" }}>{q ? "Nothing matches." : "No profiles yet."}</div>}
      </div>
      {msg && <div role="alert" style={{ padding: "6px 16px", fontSize: 12, color: "var(--negative)" }}>{msg}</div>}
      <div style={{ display: "flex", gap: 4, padding: 8, borderTop: "1px solid var(--rule)" }}>
        <button style={foot} onClick={() => api.openMain(null)}>Open Tandem</button>
        <button style={foot} onClick={() => api.openMain("settings")}>Settings</button>
        <button style={foot} onClick={() => api.quit()}>Quit</button>
      </div>
    </div>
  );
}

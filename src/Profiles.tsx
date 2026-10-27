import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { scheduleLine } from "./SessionStart";
import { api, errText, initial, type AppInfo, type AppKey, type Group, type Profile, type Schedule, type Tint, type Usage, type UsageRow } from "./api";

const GET_URL: Record<AppKey, string> = { claude: "https://claude.ai/download", codex: "https://chatgpt.com/codex", cc: "https://docs.claude.com/en/docs/claude-code/setup", cx: "https://github.com/openai/codex" };

const NEAR = 85;

function when(s: string): Date | null {
  if (!s) return null;
  const d = new Date(/^\d+$/.test(s) ? Number(s) * 1000 : s);
  return isNaN(d.getTime()) ? null : d;
}

function fmtReset(s: string): string {
  const d = when(s);
  if (!d) return "—";
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return d.toDateString() === new Date().toDateString() ? time : `${d.toLocaleDateString([], { weekday: "short" })} ${time}`;
}

/** Short and long window of a profile: Claude by label, Codex by order (its windows vary by plan). */
export function windows(u: Usage | undefined): [UsageRow | undefined, UsageRow | undefined] {
  if (!u) return [undefined, undefined];
  return u.app === "claude" ? [u.rows.find((r) => r.label === "5h"), u.rows.find((r) => r.label === "7d")] : [u.rows[0], u.rows[1]];
}

export function Meter({ r, short }: { r: UsageRow | undefined; short: string }) {
  if (!r) return <span style={{ color: "var(--faint)" }}>—</span>;
  const pct = Math.max(0, Math.min(100, Math.round(r.pct)));
  const hot = pct >= NEAR;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div className="track" role="img" aria-label={`${pct}% of the ${short} limit used`}><div className={hot ? "fill hot" : "fill"} style={{ width: `${pct}%` }} /></div>
      <span className="num" style={{ fontSize: 12, color: "var(--muted)" }}><span style={{ fontWeight: 500, color: hot ? "var(--negative)" : "var(--text)" }}>{pct}%{pct >= 100 ? " limit reached" : hot ? " near limit" : ""}</span> · resets {fmtReset(r.resetsAt)}</span>
    </div>
  );
}

export interface Row { key: string; name: string; sub: string; tint: Tint; running: boolean; signedOut: boolean; profile?: Profile }

export interface Ctx {
  reload: () => void;
  toast: (m: string) => void;
  edit: (p: Profile) => void;
  del: (p: Profile) => void;
  newProfile: (app: AppKey) => void;
  importProfiles: () => void;
  signin: (p: Profile) => void;
  afterLaunch: () => void;
  groups: Group[];
  schedules: Schedule[];
  sessionStart: (p: Profile) => void;
  newGroup: () => void;
}

export function Menu({ row, ctx, app, onClose }: { row: Row; ctx: Ctx; app: AppKey; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [picking, setPicking] = useState(false);
  useEffect(() => {
    const items = () => Array.from(ref.current?.querySelectorAll<HTMLButtonElement>("button") ?? []);
    items()[0]?.focus();
    const key = (e: KeyboardEvent) => {
      const list = items();
      const i = list.indexOf(document.activeElement as HTMLButtonElement);
      if (e.key === "Escape") { e.stopPropagation(); onClose(); }
      else if (e.key === "ArrowDown") { e.preventDefault(); list[(i + 1) % list.length]?.focus(); }
      else if (e.key === "ArrowUp") { e.preventDefault(); list[(i - 1 + list.length) % list.length]?.focus(); }
    };
    const away = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) onClose(); };
    document.addEventListener("keydown", key, true);
    document.addEventListener("mousedown", away);
    return () => { document.removeEventListener("keydown", key, true); document.removeEventListener("mousedown", away); };
  }, [onClose]);
  const run = (fn: () => void) => () => { onClose(); fn(); };
  return (
    <div ref={ref} className="menu rise" role="menu">
      {row.profile && <button role="menuitem" onClick={run(() => ctx.edit(row.profile!))}>Edit name and colour</button>}
      {row.profile && <button role="menuitem" onClick={run(() => ctx.signin(row.profile!))}>{row.signedOut ? "Sign in" : "Sign in again"}</button>}
      {row.profile && !picking && <button role="menuitem" onClick={() => setPicking(true)}>Add to group…</button>}
      {row.profile && picking && (
        <>
          {ctx.groups.map((g) => <button key={g.name} role="menuitem" onClick={run(() => api.addToGroup(g.name, app, row.name).then(() => ctx.toast(`Added to ${g.name}`)).catch((e) => ctx.toast(errText(e))))}>{g.name}</button>)}
          <button role="menuitem" onClick={run(ctx.newGroup)}>New group…</button>
        </>
      )}
      {row.profile && app === "claude" && <button role="menuitem" onClick={run(() => ctx.sessionStart(row.profile!))}>Session start (beta)…</button>}
      {row.profile && <button role="menuitem" onClick={run(() => api.setShortcut(app, row.name, !row.profile!.hasShortcut).then(() => { ctx.reload(); ctx.toast(row.profile!.hasShortcut ? "Shortcut removed" : "Shortcut added to your desktop"); }).catch((e) => ctx.toast(errText(e))))}>{row.profile.hasShortcut ? "Remove desktop shortcut" : "Create desktop shortcut"}</button>}
      <button role="menuitem" onClick={run(() => api.showFolder(app, row.name).catch((e) => ctx.toast(errText(e))))}>Show profile folder</button>
      {row.profile && (
        <>
          <div className="sep" />
          <button role="menuitem" className="bad" onClick={run(() => ctx.del(row.profile!))}>Delete profile…</button>
        </>
      )}
    </div>
  );
}

export function Profiles({ app, profiles, usage, retry, query, ctx, dataDir }: { dataDir: string; app: AppInfo; profiles: Profile[]; usage: Usage[]; retry: () => void; query: string; ctx: Ctx }) {
  const [tab, setTab] = useState<"all" | "running" | "out">("all");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<string | null>(null);
  const color = app.key === "claude" ? "var(--claude)" : "var(--codex)";

  const usageOf = (name: string) => usage.find((u) => u.name === name);
  const all: Row[] = [
    ...profiles.map((p) => ({ key: p.name, name: p.name, sub: !p.signedIn ? "Not signed in" : usageOf(p.name)?.account || "Signed in", tint: p.tint, running: p.running, signedOut: !p.signedIn, profile: p })),
  ];
  const q = query.trim().toLowerCase();
  const matched = q ? all.filter((r) => r.name.toLowerCase().includes(q) || r.sub.toLowerCase().includes(q)) : all;
  const rows = tab === "running" ? matched.filter((r) => r.running) : tab === "out" ? matched.filter((r) => r.signedOut) : matched;
  const runningCount = all.filter((r) => r.running).length;

  const best = all
    .filter((r) => !r.signedOut && windows(usageOf(r.name))[0])
    .sort((a, b) => windows(usageOf(a.name))[0]!.pct - windows(usageOf(b.name))[0]!.pct)[0];
  const offline = usage.some((u) => u.kind === "offline");
  const lastOk = Math.max(0, ...usage.map((u) => u.updated));

  const chosen = all.filter((r) => sel.has(r.key));
  const launchRows = async (list: Row[]) => {
    const todo = list.filter((r) => !r.running);
    if (!todo.length) return ctx.toast("Already running");
    try {
      for (const r of todo) await api.launch(app.key, r.name);
      ctx.toast(todo.length === 1 ? `Launching ${todo[0].name}` : `Launching ${todo.length} profiles`);
      setSel(new Set());
      ctx.afterLaunch();
    } catch (e) { ctx.toast(errText(e)); }
    ctx.reload();
  };
  const closeRow = async (r: Row) => {
    try { await api.close(app.key, r.name); ctx.toast(`Closed ${r.name}`); } catch (e) { ctx.toast(errText(e)); }
    ctx.reload();
  };
  const toggle = (k: string) => setSel((s) => { const n = new Set(s); if (n.has(k)) n.delete(k); else n.add(k); return n; });

  const launchLabel = chosen.length === 0 ? "Select profiles to launch" : chosen.length === 1 ? "Launch 1 profile" : `Launch ${chosen.length} profiles`;
  const tabs: [typeof tab, string, number][] = [["all", "All", all.length], ["running", "Running", runningCount], ["out", "Signed out", all.filter((r) => r.signedOut).length]];
  const empty = q ? `No profiles match “${query.trim()}”.` : !all.length ? "No profiles yet. Import your signed-in profiles or create a new one." : tab === "running" ? "Nothing is running. Launch a profile and it shows here." : tab === "out" ? "Every profile here is signed in." : "";

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div style={{ flexGrow: 1, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span className="dot" style={{ width: 10, height: 10, background: color }} />
            <h1 style={{ margin: 0, fontSize: 22, fontWeight: 600, letterSpacing: "-0.015em" }}>{app.name}</h1>
          </div>
          <p style={{ margin: "6px 0 0", fontSize: 14, color: "var(--muted)" }}>
            {app.installed ? `Desktop app ${app.version} · ${profiles.length} ${profiles.length === 1 ? "profile" : "profiles"} · ${runningCount} running` : "Desktop app not installed"}
          </p>
        </div>
        <button className="btn" onClick={ctx.importProfiles}>Import signed in profiles</button>
        <button className="btn" onClick={() => ctx.newProfile(app.key)}>
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden="true"><path d="M7 2.5v9M2.5 7h9" /></svg>
          New profile
        </button>
        <button className="btn primary" disabled={!chosen.length || !app.installed} onClick={() => launchRows(chosen)}>{launchLabel}</button>
      </div>

      {!app.installed && (
        <div role="status" style={{ marginTop: 20, display: "flex", alignItems: "center", gap: 14, padding: "14px 16px", borderRadius: 12, background: "var(--raised)", border: "1px solid var(--border)" }}>
          <div style={{ flexGrow: 1, fontSize: 14, lineHeight: 1.45 }}>
            <span style={{ fontWeight: 600 }}>{app.name} isn't installed.</span>
            <span style={{ color: "var(--muted)" }}> Your profiles are kept, and you can launch them once the app is installed.</span>
          </div>
          <button className="btn" onClick={() => openUrl(GET_URL[app.key]).catch((e) => ctx.toast(errText(e)))}>Get {app.name}</button>
        </div>
      )}

      {offline && (
        <div role="status" style={{ marginTop: 20, display: "flex", alignItems: "center", gap: 14, padding: "14px 16px", borderRadius: 12, background: "var(--raised)", border: "1px solid var(--border)" }}>
          <div style={{ flexGrow: 1, minWidth: 0, fontSize: 14, lineHeight: 1.45 }}>
            <span style={{ fontWeight: 600 }}>You're offline, so usage can't refresh.</span>
            <span style={{ color: "var(--muted)" }}> {lastOk ? `The figures below are from ${new Date(lastOk * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}. ` : ""}Profiles still launch normally.</span>
          </div>
          <button className="btn" onClick={retry}>Try again</button>
        </div>
      )}

      {best && (() => {
        const [u5, u7] = windows(usageOf(best.name));
        return (
          <div style={{ marginTop: 20, display: "flex", alignItems: "center", gap: 14, padding: "14px 16px", borderRadius: 12, background: "var(--raised)" }}>
            <span className={`av tint-${best.tint}`}>{initial(best.name)}</span>
            <div style={{ flexGrow: 1, minWidth: 0, fontSize: 14, lineHeight: 1.45 }}>
              <span style={{ fontWeight: 600 }}>{best.name} has the most room right now.</span>
              <span style={{ color: "var(--muted)" }}> {Math.round(u5!.pct)}% of its 5-hour limit used{u5!.resetsAt ? `, resets at ${fmtReset(u5!.resetsAt)}` : ""}{u7 ? `; ${Math.round(u7.pct)}% of the weekly limit` : ""}.</span>
            </div>
            <button className="btn" disabled={best.running || !app.installed} onClick={() => launchRows([best])}>{best.running ? "Already running" : `Launch ${best.name}`}</button>
          </div>
        );
      })()}

      <div style={{ marginTop: 22, display: "flex", gap: 24, borderBottom: "1px solid var(--border)" }} role="tablist">
        {tabs.map(([id, t, n]) => (
          <button key={id} className={tab === id ? "tab on" : "tab"} role="tab" aria-selected={tab === id} onClick={() => setTab(id)}>{t} <span className="num" style={{ color: "var(--faint)", fontWeight: 400 }}>{n}</span></button>
        ))}
      </div>

      <div className="row head" style={{ marginTop: 4 }}>
        <span /><span>Profile</span><span>Plan</span><span>Status</span><span>5-hour limit</span><span>Weekly limit</span><span /><span />
      </div>
      {rows.map((r) => {
        const dot = r.signedOut ? "out" : r.running ? "run" : "idle";
        const u = usageOf(r.name);
        const [u5, u7] = windows(u);
        const noFigures = !r.signedOut && !u5 && !u7;
        const expired = noFigures && u?.kind === "expired";
        return (
          <div key={r.key} className={sel.has(r.key) ? "row sel" : "row"}>
            <label className="ck"><input type="checkbox" checked={sel.has(r.key)} disabled={r.signedOut} onChange={() => toggle(r.key)} aria-label={`Select ${r.name}`} /></label>
            <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0 }}>
              <span className={`av tint-${r.tint}`}>{initial(r.name)}</span>
              <div style={{ display: "flex", flexDirection: "column", minWidth: 0, lineHeight: 1.3 }}>
                <span style={{ fontSize: 14, fontWeight: 500, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{r.name}</span>
                <span style={{ fontSize: 12, color: "var(--muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{r.sub}</span>
                {(() => { const l = scheduleLine(ctx.schedules.find((s) => s.app === r.profile?.app && s.name === r.name)); return l ? <span style={{ fontSize: 12, color: "var(--muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{l}</span> : null; })()}
              </div>
            </div>
            <span style={{ fontSize: 13, color: "var(--muted)" }}>{r.signedOut ? "" : u?.plan}</span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13 }}><span className={`dot ${dot}`} />{r.signedOut ? "Signed out" : r.running ? "Running" : "Idle"}</span>
            {r.signedOut ? <span style={{ color: "var(--faint)" }}>—</span> : expired ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <span style={{ fontSize: 13, fontWeight: 500 }}>Session expired</span>
                <span style={{ fontSize: 12, color: "var(--muted)" }}>Usage unknown</span>
              </div>
            ) : noFigures ? <span style={{ fontSize: 12, color: "var(--muted)", lineHeight: 1.4 }}>{u?.message || "Loading…"}</span> : <div style={{ alignSelf: "start", marginTop: 12 }}><Meter r={u5} short="5-hour" /></div>}
            {r.signedOut ? <span style={{ color: "var(--faint)" }}>—</span> : expired ? (
              <span style={{ fontSize: 12, color: "var(--muted)", lineHeight: 1.4 }}>Open it once to sign back in and refresh</span>
            ) : noFigures ? <span /> : <div style={{ alignSelf: "start", marginTop: 12 }}><Meter r={u7} short="weekly" /></div>}
            <div style={{ display: "flex", justifyContent: "flex-end" }}>
              {r.running
                ? <button className="btn" style={{ minWidth: 88, justifyContent: "center" }} onClick={() => closeRow(r)}>Close</button>
                : r.signedOut
                  ? <button className="btn" style={{ minWidth: 88, justifyContent: "center" }} disabled={!app.installed} onClick={() => ctx.signin(r.profile!)}>Sign in</button>
                  : <button className="btn" style={{ minWidth: 88, justifyContent: "center" }} disabled={!app.installed} onClick={() => launchRows([r])}>Launch</button>}
            </div>
            <div style={{ position: "relative" }}>
              <button className="icon-btn" aria-label={`More for ${r.name}`} aria-haspopup="menu" aria-expanded={menu === r.key} onClick={() => setMenu(menu === r.key ? null : r.key)}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><circle cx="3.5" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.5" cy="8" r="1.3" /></svg>
              </button>
              {menu === r.key && <Menu row={r} ctx={ctx} app={app.key} onClose={() => setMenu(null)} />}
            </div>
          </div>
        );
      })}
      {!rows.length && <div style={{ padding: "40px 0", textAlign: "center", fontSize: 14, color: "var(--muted)" }}>{empty}{!all.length && !q && <div className="mono" style={{ marginTop: 8, fontSize: 12, color: "var(--faint)" }}>Looking in {dataDir}</div>}</div>}
      {app.key === "claude" && !!rows.length && <p style={{ margin: "16px 0 0", fontSize: 12, color: "var(--muted)", lineHeight: 1.4 }}>Usage can only be read while a profile is closed. While one is open, Tandem shows the last figures it saved.</p>}
    </>
  );
}

import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errText, type AppKey, type HistoryDetail, type HistoryRow, type State } from "./api";
import { Sheet } from "./Sheet";

const FILTERS: [string, string][] = [["all", "All"], ["claude", "Claude"], ["chatgpt", "Codex"], ["cc", "Claude Code"], ["cx", "Codex CLI"]];
const SOURCE: Record<HistoryRow["kind"], string> = { cc: "Claude Code", cx: "Codex CLI", claude: "Claude desktop", chatgpt: "ChatGPT / Codex desktop" };
const AGENT: Record<HistoryRow["kind"], string> = { cc: "Claude", cx: "Codex", claude: "Claude", chatgpt: "ChatGPT" };
const when = (t: number) => (t ? new Date(t * 1000).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }) : "");
const short = (p: string) => (p.length > 44 ? "…" + p.slice(-43) : p);

function ImportExport({ state, onClose, onDone }: { state: State; onClose: () => void; onDone: (m: string) => void }) {
  const [app, setApp] = useState<AppKey>("claude");
  const profiles = state.profiles.filter((p) => p.app === app);
  const [profile, setProfile] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const chosen = profiles.find((p) => p.name === profile)?.name ?? profiles[0]?.name ?? "";
  const pick = async () => {
    const f = await open({ multiple: false, filters: [{ name: "Data export", extensions: ["zip"] }] });
    if (typeof f !== "string") return;
    setBusy(true);
    setErr("");
    api.historyImport(f, app, chosen).then((n) => onDone(`Imported ${n} ${n === 1 ? "chat" : "chats"} into ${chosen}`)).catch((e) => { setErr(errText(e)); setBusy(false); });
  };
  return (
    <Sheet title="Import a data export" onClose={onClose}>
      <p style={{ margin: "12px 0 0", fontSize: 13, lineHeight: 1.5, color: "var(--muted)" }}>Choose the zip you downloaded from {app === "claude" ? "claude.ai (Settings › Privacy › Export data)" : "ChatGPT (Settings › Data controls › Export data)"}. It is read on this computer only and the chats are filed under the profile you pick.</p>
      <div style={{ fontSize: 13, fontWeight: 500, margin: "16px 0 8px" }}>Export from</div>
      <div className="seg" role="radiogroup" aria-label="Export from">
        {([["claude", "Claude"], ["codex", "ChatGPT / Codex"]] as const).map(([k, t]) => <button key={k} role="radio" aria-checked={app === k} className={app === k ? "on" : ""} onClick={() => { setApp(k); setProfile(""); }}>{t}</button>)}
      </div>
      <label htmlFor="exp-prof" style={{ display: "block", fontSize: 13, fontWeight: 500, margin: "16px 0 8px" }}>File under profile</label>
      <select id="exp-prof" className="field" value={chosen} onChange={(e) => setProfile(e.target.value)}>
        {profiles.map((p) => <option key={p.name}>{p.name}</option>)}
      </select>
      {!profiles.length && <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>Create a {app === "claude" ? "Claude" : "Codex"} profile first.</div>}
      {err && <div role="alert" style={{ marginTop: 12, fontSize: 13, color: "var(--negative)" }}>{err}</div>}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!chosen || busy} onClick={pick}>{busy ? "Reading…" : "Choose zip file…"}</button>
      </div>
    </Sheet>
  );
}

export function History({ state, toast, reload }: { state: State; toast: (m: string) => void; reload: () => void }) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [rows, setRows] = useState<HistoryRow[]>([]);
  const [sel, setSel] = useState<string | null>(null);
  const [detail, setDetail] = useState<HistoryDetail | null>(null);
  const [busy, setBusy] = useState(true);
  const [cover, setCover] = useState<[number, number]>([0, 0]);
  const [importing, setImporting] = useState(false);

  const load = useCallback(() => {
    api.historyList(query, filter).then(setRows).catch((e) => toast(errText(e)));
    api.historyCoverage().then(setCover).catch(() => {});
  }, [query, filter, toast]);

  // Index on open and every minute; the list is refreshed after each pass.
  useEffect(() => {
    let live = true;
    const pass = () => { setBusy(true); api.historyRefresh().catch((e) => toast(errText(e))).finally(() => { if (live) { setBusy(false); load(); } }); };
    pass();
    const t = window.setInterval(pass, 60000);
    return () => { live = false; window.clearInterval(t); };
  }, [load, toast]);
  useEffect(() => { const t = window.setTimeout(load, 200); return () => window.clearTimeout(t); }, [load]);
  useEffect(() => {
    if (sel) api.historySession(sel).then(setDetail).catch((e) => toast(errText(e)));
  }, [sel, toast]);

  const installed = (k: string) => state.apps.find((a) => a.key === k)?.installed;
  const d = detail?.row.id === sel ? detail?.row : undefined;
  const copy = () => detail && navigator.clipboard.writeText(detail.messages.map((m) => `${m.role === "user" ? "You" : AGENT[detail.row.kind]}: ${m.text}`).join("\n\n")).then(() => toast("Copied")).catch(() => toast("Couldn't copy"));
  const primary = () => {
    if (!d) return;
    if (d.kind === "cc" || d.kind === "cx") api.historyResume(d.id).catch((e) => toast(errText(e)));
    else api.launch(d.kind === "claude" ? "claude" : "codex", d.profile).then(reload).catch((e) => toast(errText(e)));
  };
  const terminal = d && (d.kind === "cc" || d.kind === "cx");
  const noCli = terminal && !installed(d.kind);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "calc(100vh - 100px)", minHeight: 420 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <h1 style={{ margin: 0, fontSize: 22, fontWeight: 600, letterSpacing: "-0.015em" }}>Chat history</h1>
        <button className="btn" onClick={() => setImporting(true)}>Import export…</button>
      </div>
      <div style={{ margin: "12px 0", padding: "10px 14px", borderRadius: 12, background: "var(--raised)", fontSize: 13, color: "var(--muted)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <span>Terminal sessions update on their own. Desktop chats come from each account’s data export: {cover[0]} of {cover[1]} desktop profiles imported.</span>
        <button className="btn quiet" onClick={() => openUrl("https://support.claude.com/en/articles/9450526-how-can-i-export-my-claude-data").catch((e) => toast(errText(e)))}>How to export</button>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <input className="field" style={{ maxWidth: 320, borderRadius: 999 }} placeholder="Search all chats" aria-label="Search chats" value={query} onChange={(e) => setQuery(e.target.value)} />
        <div className="seg" role="radiogroup" aria-label="Filter">
          {FILTERS.map(([k, t]) => <button key={k} role="radio" aria-checked={filter === k} className={filter === k ? "on" : ""} onClick={() => setFilter(k)}>{t}</button>)}
        </div>
        <span style={{ fontSize: 13, color: "var(--muted)" }}>{busy ? "Indexing…" : `${rows.length}${rows.length === 300 ? "+" : ""} ${rows.length === 1 ? "chat" : "chats"}`}</span>
      </div>
      <div style={{ display: "flex", gap: 16, marginTop: 12, flex: 1, minHeight: 0 }}>
        <div role="listbox" aria-label="Chats" style={{ width: 380, flex: "none", overflowY: "auto", paddingRight: 4 }}>
          {rows.map((r) => (
            <button key={r.id} role="option" aria-selected={sel === r.id} onClick={() => setSel(r.id)}
              style={{ display: "block", width: "100%", textAlign: "left", padding: "10px 12px", marginBottom: 2, border: 0, borderRadius: 8, background: sel === r.id ? "var(--active)" : "transparent", color: "var(--text)", cursor: "pointer" }}>
              <div style={{ fontSize: 14, fontWeight: 500, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{r.title}</div>
              <div style={{ fontSize: 12, color: "var(--muted)", marginTop: 2 }}>{SOURCE[r.kind]} · {r.profile} · {when(r.updated)} · {r.count} msgs</div>
              {r.snippet && <div style={{ fontSize: 12, color: "var(--muted)", marginTop: 4, lineHeight: 1.4 }}>{r.snippet}</div>}
            </button>
          ))}
          {!rows.length && !busy && <div style={{ padding: "40px 12px", textAlign: "center", fontSize: 14, color: "var(--muted)" }}>{query ? "Nothing matches." : "No chats yet. Terminal sessions appear here once you use Claude Code or Codex CLI; import an export for desktop chats."}</div>}
        </div>
        <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", border: "1px solid var(--border)", borderRadius: 12, background: "var(--surface)" }}>
          {!d || !detail ? <div style={{ margin: "auto", fontSize: 14, color: "var(--muted)" }}>Select a chat to read it.</div> : (
            <>
              <div style={{ padding: "16px 20px", borderBottom: "1px solid var(--rule)", display: "flex", alignItems: "flex-start", gap: 12 }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 16, fontWeight: 600 }}>{d.title}</div>
                  <div style={{ fontSize: 12, color: "var(--muted)", marginTop: 4 }}>{d.profile} · {SOURCE[d.kind]} · {when(d.updated)} · {d.count} messages{d.cwd ? ` · ${short(d.cwd)}` : ""}</div>
                </div>
                <button className="btn" onClick={copy}>Copy</button>
                <button className="btn primary" disabled={!!noCli} title={noCli ? `${d.kind === "cc" ? "Claude Code" : "Codex CLI"} wasn’t found on this PC` : undefined} onClick={primary}>{terminal ? "Resume in terminal" : `Open ${d.profile}`}</button>
              </div>
              <div style={{ flex: 1, overflowY: "auto", padding: "8px 20px 20px" }}>
                {detail.messages.map((m, i) => (
                  <div key={i} style={{ marginTop: 16 }}>
                    <div style={{ fontSize: 12, fontWeight: 600, color: m.role === "user" ? "var(--text)" : "var(--muted)" }}>{m.role === "user" ? "You" : AGENT[d.kind]}</div>
                    <div style={{ fontSize: 14, lineHeight: 1.55, whiteSpace: "pre-wrap", wordBreak: "break-word", marginTop: 2 }}>{m.text}</div>
                  </div>
                ))}
                {!detail.shownAll && <div style={{ marginTop: 16, fontSize: 12, color: "var(--muted)" }}>Showing the first {detail.messages.length} of {d.count} messages.</div>}
              </div>
            </>
          )}
        </div>
      </div>
      {importing && <ImportExport state={state} onClose={() => setImporting(false)} onDone={(m) => { setImporting(false); toast(m); load(); }} />}
    </div>
  );
}

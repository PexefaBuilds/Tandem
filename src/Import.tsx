import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText, initial, TINTS, type AppKey, type Found, type Tint } from "./api";
import { Sheet } from "./Sheet";

export interface Item extends Found { checked: boolean; name: string; tint: Tint; error: string; done: boolean }

const APP_NAME: Record<AppKey, string> = { claude: "Claude", codex: "Codex", cc: "Claude Code", cx: "Codex CLI" };
// Upper bound: the walk stops at 12 s, the wait at 14 s, and checking which profiles are open takes a few more.
const SCAN_BUDGET_S = 20;
const PALETTE = TINTS.filter((t) => t !== "graphite"); // graphite stays out of the default rotation

export function useImporter(installed: Record<AppKey, boolean>) {
  const [items, setItems] = useState<Item[]>([]);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");

  const merge = useCallback((found: Found[], none: string) => {
      setNote(found.length ? "" : none);
      setItems((cur) => {
        const fresh = found.filter((f) => !cur.some((c) => c.path === f.path));
        // A profile seen again may have been closed since: take the new in-use state.
        cur = cur.map((c) => { const f = found.find((x) => x.path === c.path); return f && f.inUse !== c.inUse ? { ...c, inUse: f.inUse, checked: c.checked || (!f.inUse && installed[c.app]) } : c; });
        return [...cur, ...fresh.map((f, i) => ({ ...f, checked: !f.inUse && installed[f.app], tint: PALETTE[(cur.length + i) % PALETTE.length], error: "", done: false }))];
      });
  }, [installed]);

  const add = async () => {
    const dir = await open({ directory: true, title: "Choose a profile folder, or a folder holding several" });
    if (typeof dir !== "string") return;
    try { merge(await api.scanFolder(dir), "No Claude or Codex profiles found there."); } catch (e) { setNote(errText(e)); }
  };

  const [scanning, setScanning] = useState(false);
  const [scanMs, setScanMs] = useState(0);
  useEffect(() => {
    if (!scanning) return;
    const t0 = Date.now();
    const t = window.setInterval(() => setScanMs(Date.now() - t0), 250);
    return () => window.clearInterval(t);
  }, [scanning]);
  const scan = async () => {
    setScanMs(0); setScanning(true); setNote("");
    try { merge(await api.scanComputer(), "No signed-in profiles found on this computer."); } catch (e) { setNote(errText(e)); }
    setScanning(false);
  };

  /** Re-checks which listed profiles are still open, so closing Claude and pressing this is enough. */
  const refresh = async () => {
    setNote("");
    try {
      const open = await api.recheckInUse(items.map((i) => [i.path, i.app]));
      setItems((cur) => cur.map((c, n) => (open[n] === undefined || open[n] === c.inUse ? c : { ...c, inUse: open[n], checked: c.checked || (!open[n] && installed[c.app]) })));
    } catch (e) { setNote(errText(e)); }
  };

  // Live name check; the backend owns the rules.
  const key = items.map((i) => `${i.app}:${i.name}`).join("|");
  useEffect(() => {
    let live = true;
    Promise.all(items.map((i) => api.checkName(i.app, i.name, null).catch(() => null))).then((errs) => {
      if (live) setItems((cur) => cur.map((c, n) => (c.done || c.error.startsWith("Copy") ? c : { ...c, error: errs[n] ?? "" })));
    });
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const patch = (path: string, p: Partial<Item>) => setItems((cur) => cur.map((c) => (c.path === path ? { ...c, ...p } : c)));
  const chosen = items.filter((i) => i.checked && !i.done);
  const ready = chosen.length > 0 && chosen.every((i) => !i.error && !i.inUse);

  /** Copies each chosen profile in turn; returns how many succeeded. */
  const run = async () => {
    setBusy(true);
    let ok = 0;
    for (const i of chosen) {
      patch(i.path, { error: "Copying…" });
      try { await api.importProfile(i.path, i.app, i.name, i.tint); patch(i.path, { done: true, error: "" }); ok++; }
      catch (e) { patch(i.path, { error: errText(e) }); }
    }
    setBusy(false);
    return ok;
  };
  return { items, add, refresh, scan, scanning, scanMs, patch, chosen, ready, busy, run, note };
}

/** The scan stops after SCAN_BUDGET_S at the latest, so the bar is an upper bound; it may finish sooner. */
function ScanProgress({ ms }: { ms: number }) {
  const s = Math.floor(ms / 1000);
  const left = Math.max(0, SCAN_BUDGET_S - s);
  return (
    <div role="status" style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 12 }}>
      <span className="spinner" aria-hidden="true" />
      <div style={{ flexGrow: 1, minWidth: 0 }}>
        <div className="num" style={{ fontSize: 13 }}>Scanning this computer… {s} s <span style={{ color: "var(--muted)" }}>· {left > 0 ? `at most ${left} s left` : "wrapping up"}</span></div>
        <div className="track" style={{ marginTop: 6 }} role="progressbar" aria-valuemin={0} aria-valuemax={SCAN_BUDGET_S} aria-valuenow={Math.min(s, SCAN_BUDGET_S)} aria-label="Scan progress"><div className="fill" style={{ width: `${Math.min(100, (ms / (SCAN_BUDGET_S * 1000)) * 100)}%`, transition: "width .25s linear" }} /></div>
      </div>
    </div>
  );
}

export function ImportList({ imp }: { imp: ReturnType<typeof useImporter> }) {
  return (
    <div>
      {imp.items.map((i) => {
        const sub = i.done ? "Brought over" : i.inUse ? "Open right now. Quit it (also from the tray), then press Refresh." : i.error || `${APP_NAME[i.app]} · ${i.isDefault ? "your usual login" : i.hasSignIn ? "has sign-in data" : "no sign-in data"}`;
        const bad = !i.done && (i.inUse || (i.error && i.error !== "Copying…"));
        return (
          <div key={i.path} style={{ display: "flex", alignItems: "center", gap: 12, minHeight: 52, padding: "6px 0" }}>
            <input type="checkbox" checked={i.checked && !i.inUse} disabled={i.inUse || i.done || imp.busy} onChange={(e) => imp.patch(i.path, { checked: e.target.checked })} aria-label={`Bring over ${i.name}`} style={{ width: 18, height: 18, margin: 0, accentColor: "var(--text)" }} />
            <span className={`av tint-${i.tint}`}>{initial(i.name)}</span>
            <div style={{ flexGrow: 1, minWidth: 0 }}>
              <input className="field" style={{ height: 30, fontSize: 14, fontWeight: 500 }} value={i.name} disabled={i.done || imp.busy} onChange={(e) => imp.patch(i.path, { name: e.target.value })} aria-label="Profile name" />
              <div role="status" style={{ marginTop: 3, fontSize: 12, color: bad ? "var(--negative)" : "var(--muted)" }}>{sub}</div>
            </div>
          </div>
        );
      })}
      {imp.scanning && <ScanProgress ms={imp.scanMs} />}
      {imp.note && <p role="status" style={{ margin: "8px 0 0", fontSize: 13, color: "var(--muted)" }}>{imp.note}</p>}
      <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
        <button className="btn" onClick={imp.scan} disabled={imp.busy || imp.scanning}>{imp.scanning ? "Scanning…" : "Scan this computer"}</button>
        {imp.items.length > 0 && <button className="btn" onClick={imp.refresh} disabled={imp.busy || imp.scanning}>Refresh</button>}
        <button className="btn" onClick={imp.add} disabled={imp.busy}>Add a folder…</button>
      </div>
    </div>
  );
}

export function ImportSheet({ installed, onClose, onDone }: { installed: Record<AppKey, boolean>; onClose: () => void; onDone: (m: string) => void }) {
  const imp = useImporter(installed);
  const go = async () => { const n = await imp.run(); if (n) onDone(n === 1 ? "Brought over 1 profile" : `Brought over ${n} profiles`); };
  return (
    <Sheet title="Import signed in profiles" onClose={onClose}>
      <p style={{ margin: "8px 0 12px", fontSize: 14, color: "var(--muted)", lineHeight: 1.5 }}>Copies a profile folder into Tandem and keeps its sign-in. The original isn't changed. Caches and logs are left out.</p>
      <ImportList imp={imp} />
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
        <button className="btn" onClick={onClose}>Close</button>
        <button className="btn primary" disabled={!imp.ready || imp.busy} onClick={go}>{imp.chosen.length === 1 ? "Bring over 1 profile" : `Bring over ${imp.chosen.length} profiles`}</button>
      </div>
    </Sheet>
  );
}

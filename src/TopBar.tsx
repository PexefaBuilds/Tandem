import { useEffect, useState, type CSSProperties } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri, isMac } from "./platform";

const win = () => (inTauri ? getCurrentWindow() : null);
const capBtn: CSSProperties = { width: 46, height: 48, display: "inline-flex", alignItems: "center", justifyContent: "center", border: 0, background: "transparent", color: "inherit", cursor: "default" };

export function Caption() {
  return (
    <div style={{ display: "flex", alignItems: "center", marginLeft: 8, marginRight: -8, color: "var(--muted)" }}>
      <button style={capBtn} aria-label="Minimize" onClick={() => win()?.minimize()}><svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" /></svg></button>
      <button style={capBtn} aria-label="Maximize" onClick={() => win()?.toggleMaximize()}><svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" /></svg></button>
      <button style={capBtn} aria-label="Close" onClick={() => win()?.close()}><svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 0l10 10M10 0 0 10" stroke="currentColor" /></svg></button>
    </div>
  );
}

function ago(updated: number, busy: boolean) {
  if (busy) return "Refreshing usage…";
  if (!updated) return "Usage not loaded yet";
  const m = Math.floor((Date.now() / 1000 - updated) / 60);
  return m < 1 ? "Usage updated just now" : `Usage updated ${m} min ago`;
}

export function TopBar({ onRefresh, updated, busy }: { onRefresh: () => void; updated: number; busy: boolean }) {
  const [, tick] = useState(0);
  useEffect(() => { const t = window.setInterval(() => tick((n) => n + 1), 30000); return () => window.clearInterval(t); }, []);
  return (
    <div data-tauri-drag-region style={{ height: 48, flex: "none", display: "flex", alignItems: "center", gap: 12 }}>
      <div style={{ flexGrow: 1 }} />
      <span style={{ fontSize: 12, color: "var(--muted)" }}>{ago(updated, busy)}</span>
      <button className="icon-btn" aria-label="Refresh usage" disabled={busy} onClick={onRefresh}>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" /><path d="M13.5 2.5v3h-3" /></svg>
      </button>
      {!isMac && <Caption />}
    </div>
  );
}

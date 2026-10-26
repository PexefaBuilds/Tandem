import type { AppKey, State } from "./api";
import { ImportList, useImporter } from "./Import";
import { Caption } from "./TopBar";
import { isMac } from "./platform";

export function Welcome({ state, onFinish }: { state: State; onFinish: (imported: number) => void }) {
  const installed = Object.fromEntries(state.apps.map((a) => [a.key, a.installed])) as Record<AppKey, boolean>;
  const imp = useImporter(installed);
  const n = imp.chosen.length;
  const go = async () => onFinish(n ? await imp.run() : 0);
  return (
    <div style={{ position: "absolute", inset: 0, zIndex: 45, background: "var(--ground)", display: "flex", alignItems: "center", justifyContent: "center", overflow: "auto" }}>
      <div data-tauri-drag-region style={{ position: "absolute", top: 0, left: 0, right: 0, height: 48, display: "flex", justifyContent: "flex-end", paddingRight: 8 }}>{!isMac && <Caption />}</div>
      <div className="rise" style={{ width: 540, display: "flex", flexDirection: "column", alignItems: "center", padding: "24px 0" }}>
        <svg width="64" height="64" viewBox="0 0 48 48" aria-hidden="true">
          <rect x="7" y="4" width="15" height="32" rx="7.5" fill="var(--claude)" />
          <rect x="26" y="12" width="15" height="32" rx="7.5" fill="var(--codex)" />
        </svg>
        <h1 style={{ margin: "20px 0 0", fontSize: 28, fontWeight: 600, letterSpacing: "-0.02em", textAlign: "center" }}>Welcome to Tandem</h1>
        <p style={{ margin: "8px 0 0", fontSize: 15, color: "var(--muted)", textAlign: "center", lineHeight: 1.5 }}>Your Claude and Codex accounts, in tandem. Each one keeps its own sign-in.</p>

        <div style={{ marginTop: 28, width: "100%", background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 16, padding: "18px 20px" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>Apps on this computer</div>
          <div style={{ marginTop: 10 }}>
            {state.apps.map((a) => (
              <div key={a.key} style={{ display: "grid", gridTemplateColumns: "8px minmax(0,1fr) 120px 120px", columnGap: 12, alignItems: "center", height: 36, fontSize: 14 }}>
                <span className="dot" style={{ background: a.key === "claude" ? "var(--claude)" : "var(--codex)" }} />
                <span>{a.name}</span>
                <span className="num" style={{ fontSize: 13, color: "var(--muted)" }}>{a.installed ? a.version : "—"}</span>
                <span style={{ fontSize: 13, textAlign: "right", color: a.installed ? "var(--positive)" : "var(--muted)" }}>{a.installed ? "Found" : "Not installed"}</span>
              </div>
            ))}
          </div>
        </div>

        <div style={{ marginTop: 12, width: "100%", background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 16, padding: "18px 20px" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>Bring over your profiles</div>
          <div style={{ margin: "4px 0 8px", fontSize: 13, color: "var(--muted)", lineHeight: 1.45 }}>Pick a folder you already use. Profiles are copied and keep their sign-ins. Your original folders aren't changed.</div>
          <ImportList imp={imp} />
        </div>

        <div style={{ marginTop: 24, width: "100%", display: "flex", justifyContent: "center", gap: 10 }}>
          <button className="btn" disabled={imp.busy} onClick={() => onFinish(0)}>Start fresh</button>
          <button className="btn primary" disabled={imp.busy || (n > 0 && !imp.ready)} onClick={go}>{n === 0 ? "Continue" : n === 1 ? "Bring over 1 profile" : `Bring over ${n} profiles`}</button>
        </div>
      </div>
    </div>
  );
}

import type { CSSProperties, ReactNode } from "react";
import { isMac } from "./platform";
import type { State } from "./api";

export type Route = "claude" | "codex" | "cc" | "cx" | "groups" | "history" | "settings";

const svg = { width: 16, height: 16, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": true } as const;
const section = (top: number): CSSProperties => ({ fontSize: 12, color: "var(--muted)", padding: `${top}px 10px 6px` });

function Nav({ label, count, on, icon, onClick }: { label: string; count?: number; on?: boolean; icon: ReactNode; onClick: () => void }) {
  return (
    <button className={on ? "nav on" : "nav"} aria-current={on ? "page" : undefined} onClick={onClick}>
      {icon}
      <span style={{ flexGrow: 1 }}>{label}</span>
      {count !== undefined && <span className="num" style={{ fontSize: 12, color: "var(--faint)" }}>{count}</span>}
    </button>
  );
}
const dot = (c: string) => <span className="dot" style={{ background: c }} />;
const term = (c: string) => <svg {...svg} stroke={c} strokeWidth="1.6"><path d="M3 4.5 6.5 8 3 11.5" /><path d="M8.5 11.5H13" /></svg>;

export function Sidebar({ route, onNav, state }: { route: Route; onNav: (r: Route) => void; state: State | null }) {
  const n = (app: string) => (state ? state.profiles.filter((p) => p.app === app).length : undefined);
  const total = state ? state.profiles.length : 0;
  return (
    <aside style={{ width: 228, flex: "none", display: "flex", flexDirection: "column", padding: "0 10px 12px" }}>
      <div data-tauri-drag-region style={{ height: 48, display: "flex", alignItems: "center", padding: "0 10px" }}>
        {isMac && <div style={{ width: 70 }} aria-hidden="true" />}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "4px 10px 18px" }}>
        <svg width="24" height="24" viewBox="5 2 38 44" aria-hidden="true">
          <rect x="7" y="4" width="15" height="32" rx="7.5" fill="var(--claude)" />
          <rect x="26" y="12" width="15" height="32" rx="7.5" fill="var(--codex)" />
        </svg>
        <div style={{ display: "flex", flexDirection: "column", lineHeight: 1.15 }}>
          <span style={{ fontSize: 15, fontWeight: 600, letterSpacing: "-0.025em" }}>Tandem</span>
          <span style={{ fontSize: 12, color: "var(--muted)" }}>0.1 · {total} {total === 1 ? "profile" : "profiles"}</span>
        </div>
      </div>

      <div style={section(6)}>Desktop apps</div>
      <Nav on={route === "claude"} onClick={() => onNav("claude")} label="Claude" count={n("claude")} icon={dot("var(--claude)")} />
      <Nav on={route === "codex"} onClick={() => onNav("codex")} label="Codex" count={n("codex")} icon={dot("var(--codex)")} />

      <div style={section(20)}>Terminal</div>
      <Nav on={route === "cc"} onClick={() => onNav("cc")} label="Claude Code" count={n("cc")} icon={term("var(--claude)")} />
      <Nav on={route === "cx"} onClick={() => onNav("cx")} label="Codex CLI" count={n("cx")} icon={term("var(--codex)")} />

      <div style={section(20)}>Launch together</div>
      <Nav on={route === "groups"} onClick={() => onNav("groups")} label="Groups" icon={<svg {...svg} strokeWidth="1.5"><rect x="2" y="2" width="5" height="5" rx="1.5" /><rect x="9" y="2" width="5" height="5" rx="1.5" /><rect x="2" y="9" width="5" height="5" rx="1.5" /><rect x="9" y="9" width="5" height="5" rx="1.5" /></svg>} />
      <Nav on={route === "history"} onClick={() => onNav("history")} label="Chat history" icon={<svg {...svg} strokeWidth="1.5"><path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z" /></svg>} />

      <div style={{ flexGrow: 1 }} />
      <Nav on={route === "settings"} onClick={() => onNav("settings")} label="Settings" icon={<svg {...svg} strokeWidth="1.5"><circle cx="8" cy="8" r="2.2" /><path d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1" /></svg>} />
    </aside>
  );
}

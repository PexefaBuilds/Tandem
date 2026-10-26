import { useEffect, useRef, useState } from "react";
import { api, errText, initial, type Profile, type SignInStatus } from "./api";
import { Sheet } from "./Sheet";

const APP: Record<string, string> = { claude: "Claude", codex: "Codex" };
const row = { display: "flex", alignItems: "center", gap: 12, marginTop: 16 } as const;

export function SignIn({ p, onClose, onDone, onLaunch }: { p: Profile; onClose: () => void; onDone: (m: string) => void; onLaunch: (p: Profile) => void }) {
  const [started, setStarted] = useState(false);
  const [st, setSt] = useState<SignInStatus>({ phase: "idle", message: "" });
  const [err, setErr] = useState("");
  const alive = useRef(true);
  useEffect(() => () => { alive.current = false; }, []);

  const start = async () => {
    setErr("");
    try { await api.signinStart(p.app, p.name); setStarted(true); setSt({ phase: "waiting", message: "Starting…" }); } catch (e) { setErr(errText(e)); }
  };
  useEffect(() => {
    if (!started) return;
    const t = window.setInterval(() => api.signinStatus().then((s) => alive.current && setSt(s)).catch(() => {}), 1000);
    return () => window.clearInterval(t);
  }, [started]);

  const claude = p.app === "claude";
  const phase = st.phase;
  const finished = started && (phase === "ready" || phase === "failed" || phase === "cancelled");
  const cancel = () => { if (started && !finished) api.signinCancel().catch(() => {}); else onClose(); };

  if (!started && !err) {
    return (
      <Sheet title={`Sign in to ${p.name}`} onClose={onClose}>
        <div style={row}><span className={`av tint-${p.tint}`}>{initial(p.name)}</span><span style={{ fontSize: 14, fontWeight: 500 }}>{p.name}</span></div>
        <p style={{ margin: "16px 0 0", fontSize: 14, color: "var(--muted)", lineHeight: 1.55 }}>
          {claude
            ? "Here's what happens: your Main Claude closes and its session is set aside. A blank Claude opens, and you sign in with the account you want. Tandem saves that session into this profile and puts your Main one back."
            : "A fresh Codex window opens for this profile. Sign in there with the ChatGPT account you want it to use. Tandem notices as soon as you're done."}
        </p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" autoFocus onClick={start}>Continue</button>
        </div>
      </Sheet>
    );
  }

  if (err || phase === "failed" || phase === "cancelled") {
    const msg = err || (phase === "cancelled" ? "Sign-in was cancelled. Nothing was changed." : st.message);
    return (
      <Sheet title="Sign-in didn't finish" onClose={onClose}>
        <p role="alert" style={{ margin: "12px 0 0", fontSize: 14, color: "var(--muted)", lineHeight: 1.55 }}>{msg} {p.name} is still signed out.</p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
          <button className="btn" onClick={onClose}>Close</button>
          <button className="btn primary" autoFocus onClick={() => { setStarted(false); setErr(""); setSt({ phase: "idle", message: "" }); start(); }}>Try again</button>
        </div>
      </Sheet>
    );
  }

  if (phase === "ready") {
    return (
      <Sheet title={`${p.name} is ready`} onClose={() => onDone(`${p.name} is signed in`)}>
        <p style={{ margin: "12px 0 0", fontSize: 14, color: "var(--muted)" }}>Signed in. The profile keeps this account from now on.</p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
          <button className="btn" onClick={() => onDone(`${p.name} is signed in`)}>Done</button>
          <button className="btn primary" autoFocus onClick={() => { onDone(`${p.name} is signed in`); onLaunch(p); }}>Launch {p.name}</button>
        </div>
      </Sheet>
    );
  }

  return (
    <Sheet title={`Sign in to ${APP[p.app]}`} onClose={cancel}>
      <div style={row}>
        <span className={`av tint-${p.tint}`}>{initial(p.name)}</span>
        <div><div style={{ fontSize: 14, fontWeight: 500 }}>{p.name}</div><div role="status" style={{ fontSize: 12, color: "var(--muted)", marginTop: 2 }}>{st.message || "Waiting for you to sign in…"}</div></div>
      </div>
      <div className="wait" style={{ marginTop: 16 }} role="progressbar" aria-label="Waiting for sign-in" />
      <p style={{ margin: "16px 0 0", fontSize: 14, color: "var(--muted)", lineHeight: 1.55 }}>
        {claude ? "A blank Claude window has opened. Sign in there with the account you want this profile to use. " : "A fresh Codex window has opened. Sign in there. "}
        This window notices as soon as you're done.
      </p>
      <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 24 }}>
        <button className="btn" onClick={cancel}>Cancel sign-in</button>
      </div>
    </Sheet>
  );
}

/** Shown at start-up when an earlier sign-in set the Main session aside and never put it back. */
export function Recover({ onDone }: { onDone: (m: string) => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const go = async (restore: boolean) => {
    setBusy(true);
    try { await api.signinRecover(restore); onDone(restore ? "Your Main session is back" : "Discarded"); } catch (e) { setErr(errText(e)); setBusy(false); }
  };
  return (
    <Sheet title="A sign-in was interrupted" onClose={() => {}}>
      <p style={{ margin: "12px 0 0", fontSize: 14, color: "var(--muted)", lineHeight: 1.55 }}>Your Main Claude session was set aside and never put back. Restoring it is recommended. Deleting it signs your Main Claude out for good.</p>
      <div role="alert" style={{ minHeight: 18, marginTop: 6, fontSize: 12, color: "var(--negative)" }}>{err}</div>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
        <button className="btn" disabled={busy} onClick={() => go(false)}>Delete it</button>
        <button className="btn primary" autoFocus disabled={busy} onClick={() => go(true)}>Restore it</button>
      </div>
    </Sheet>
  );
}

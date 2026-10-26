import { useEffect, useState, type ReactNode } from "react";
import { api, errText, initial, TINTS, type AppKey, type Profile, type Tint } from "./api";

export function Sheet({ title, children, onClose, width = 480 }: { title: string; children: ReactNode; onClose: () => void; width?: number }) {
  return (
    <div role="presentation" onMouseDown={(e) => e.target === e.currentTarget && onClose()} style={{ position: "fixed", inset: 0, background: "var(--overlay)", zIndex: 30, display: "flex", alignItems: "center", justifyContent: "center" }}>
      <div role="dialog" aria-modal="true" aria-label={title} className="rise" style={{ width, maxWidth: "calc(100vw - 32px)", maxHeight: "calc(100vh - 32px)", overflowY: "auto", padding: 24, borderRadius: 16, background: "var(--surface)", border: "1px solid var(--border)", boxShadow: "0 12px 32px rgba(0,0,0,.16),0 2px 6px rgba(0,0,0,.06)" }}>
        <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600, letterSpacing: "-0.01em" }}>{title}</h2>
        {children}
      </div>
    </div>
  );
}

const label: React.CSSProperties = { display: "block", fontSize: 13, fontWeight: 500, margin: "18px 0 8px" };
const actions: React.CSSProperties = { display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 };

function Colours({ value, onChange, letter }: { value: Tint; onChange: (t: Tint) => void; letter: string }) {
  return (
    <div role="radiogroup" aria-label="Colour" style={{ display: "flex", gap: 10 }}>
      {TINTS.map((t) => (
        <button key={t} type="button" role="radio" aria-checked={value === t} aria-label={t} className={`sw tint-${t}${value === t ? " on" : ""}`} onClick={() => onChange(t)}>{letter}</button>
      ))}
    </div>
  );
}

/** Live validation comes from the backend so the rules live in one place. */
function useNameError(app: AppKey, name: string, current: string | null) {
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    api.checkName(app, name, current).then((e) => live && setErr(e)).catch(() => {});
    return () => { live = false; };
  }, [app, name, current]);
  return name.length === 0 ? null : err; // stay quiet until the user types
}

export function NewProfile({ app: initialApp, apps, onClose, onDone, onSignIn }: { app: AppKey; apps: { key: AppKey; name: string }[]; onClose: () => void; onDone: (m: string) => void; onSignIn: (app: AppKey, name: string) => void }) {
  const [app, setApp] = useState(initialApp);
  const [name, setName] = useState("");
  const [tint, setTint] = useState<Tint>("blue");
  const [busy, setBusy] = useState(false);
  const [fail, setFail] = useState("");
  const [signin, setSignin] = useState(true);
  const err = useNameError(app, name, null);
  const valid = name.trim().length > 0 && !err && !busy;
  const submit = async () => {
    setBusy(true);
    try { await api.create(app, name, tint); onDone(`Created ${name.trim()}`); if (signin) onSignIn(app, name.trim()); } catch (e) { setFail(errText(e)); setBusy(false); }
  };
  return (
    <Sheet title="New profile" onClose={onClose}>
      <form onSubmit={(e) => { e.preventDefault(); if (valid) submit(); }}>
        <span style={label}>App</span>
        <div className="seg" role="radiogroup" aria-label="App">
          {apps.map((a) => <button key={a.key} type="button" role="radio" aria-checked={app === a.key} className={app === a.key ? "on" : ""} onClick={() => setApp(a.key)}>{a.name}</button>)}
        </div>
        <label htmlFor="pname" style={label}>Name</label>
        <input id="pname" className="field" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="Work, Personal, Client A…" aria-invalid={!!err} aria-describedby="pname-msg" autoComplete="off" />
        <div id="pname-msg" role="status" style={{ minHeight: 18, marginTop: 6, fontSize: 12, color: "var(--negative)" }}>{err ?? fail}</div>
        <span style={label}>Colour</span>
        <Colours value={tint} onChange={setTint} letter={initial(name || "A")} />
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 20 }}>
          <div style={{ flexGrow: 1 }}><div style={{ fontSize: 14, fontWeight: 500 }}>Sign in now</div><div style={{ fontSize: 12, color: "var(--muted)", marginTop: 2 }}>Opens a fresh {apps.find((a) => a.key === app)?.name} window to sign in</div></div>
          <button type="button" className={signin ? "switch on" : "switch"} role="switch" aria-checked={signin} aria-label="Sign in now" onClick={() => setSignin(!signin)} />
        </div>
        <div style={actions}>
          <button type="button" className="btn" onClick={onClose}>Cancel</button>
          <button type="submit" className="btn primary" disabled={!valid}>Create profile</button>
        </div>
      </form>
    </Sheet>
  );
}

export function EditProfile({ p, onClose, onDone }: { p: Profile; onClose: () => void; onDone: (m: string) => void }) {
  const [name, setName] = useState(p.name);
  const [tint, setTint] = useState<Tint>(p.tint);
  const [fail, setFail] = useState("");
  const err = useNameError(p.app, name, p.name);
  const valid = name.trim().length > 0 && !err && (name.trim() !== p.name || tint !== p.tint);
  const submit = async () => {
    try { await api.update(p.app, p.name, name, tint); onDone("Saved"); } catch (e) { setFail(errText(e)); }
  };
  return (
    <Sheet title="Edit profile" onClose={onClose}>
      <form onSubmit={(e) => { e.preventDefault(); if (valid) submit(); }}>
        <label htmlFor="ename" style={label}>Name</label>
        <input id="ename" className="field" autoFocus value={name} onChange={(e) => setName(e.target.value)} aria-invalid={!!err} aria-describedby="ename-msg" autoComplete="off" />
        <div id="ename-msg" role="status" style={{ minHeight: 18, marginTop: 6, fontSize: 12, color: "var(--negative)" }}>{err ?? fail}</div>
        <span style={label}>Colour</span>
        <Colours value={tint} onChange={setTint} letter={initial(name)} />
        {p.running && <p style={{ margin: "16px 0 0", fontSize: 13, color: "var(--muted)" }}>This profile is open, so the new name takes effect next launch.</p>}
        <div style={actions}>
          <button type="button" className="btn" onClick={onClose}>Cancel</button>
          <button type="submit" className="btn primary" disabled={!valid}>Save</button>
        </div>
      </form>
    </Sheet>
  );
}

export function DeleteProfile({ p, onClose, onDone }: { p: Profile; onClose: () => void; onDone: (m: string) => void }) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [fail, setFail] = useState("");
  const ok = typed === p.name && !busy;
  const submit = async () => {
    setBusy(true);
    try { await api.remove(p.app, p.name); onDone(`Deleted ${p.name}`); } catch (e) { setFail(errText(e)); setBusy(false); }
  };
  return (
    <Sheet title={`Delete ${p.name}?`} onClose={onClose}>
      <form onSubmit={(e) => { e.preventDefault(); if (ok) submit(); }}>
        <p style={{ margin: "12px 0 0", fontSize: 14, color: "var(--muted)", lineHeight: 1.5 }}>
          This removes the profile and its folder, including its sign-in and any chats stored there. It can't be undone.
        </p>
        <label htmlFor="dname" style={label}>Type <span style={{ fontWeight: 600 }}>{p.name}</span> to confirm</label>
        <input id="dname" className="field" autoFocus value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" />
        <div role="status" style={{ minHeight: 18, marginTop: 6, fontSize: 12, color: "var(--negative)" }}>{fail}</div>
        <div style={actions}>
          <button type="button" className="btn" onClick={onClose}>Cancel</button>
          <button type="submit" className="btn danger" disabled={!ok}>{p.running ? "Close and delete" : "Delete profile"}</button>
        </div>
      </form>
    </Sheet>
  );
}

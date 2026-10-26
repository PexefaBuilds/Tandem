import { useState } from "react";
import { api, errText, type Profile, type Schedule } from "./api";
import { Sheet } from "./Sheet";

const clock = (d: Date) => d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
const FIVE_H = 5 * 3600 * 1000;

function nextRun(s: Schedule): Date | null {
  if (!s.enabled || !/^\d\d:\d\d$/.test(s.time)) return null;
  const [h, m] = s.time.split(":").map(Number);
  const d = new Date();
  const today = d.toLocaleDateString("en-CA");
  d.setHours(h, m, 0, 0);
  if (d.getTime() < Date.now() - 10 * 60000 || s.lastDay === today) {
    if (s.repeat === "once" && s.lastDay === today) return null;
    d.setDate(d.getDate() + 1);
  }
  return d;
}

/** A short status for a profile row: the next start, or the open window and when it ends. */
export function scheduleLine(s: Schedule | undefined): string {
  if (!s) return "";
  const end = s.lastSent ? new Date(s.lastSent * 1000 + FIVE_H) : null;
  if (end && end.getTime() > Date.now()) return `Session window ends ${clock(end)}`;
  const n = nextRun(s);
  if (!n) return "";
  const mins = Math.max(0, Math.round((n.getTime() - Date.now()) / 60000));
  const left = mins >= 60 ? `${Math.floor(mins / 60)}h ${mins % 60}m` : `${mins}m`;
  return `Session starts ${clock(n)} · in ${left}`;
}

export function SessionStart({ p, schedule, onClose, onDone }: { p: Profile; schedule: Schedule | undefined; onClose: () => void; onDone: (m: string) => void }) {
  const [enabled, setEnabled] = useState(schedule?.enabled ?? true);
  const [time, setTime] = useState(schedule?.time || "08:00");
  const [repeat, setRepeat] = useState(schedule?.repeat || "daily");
  const [message, setMessage] = useState(schedule?.message || "hi");
  const [err, setErr] = useState("");
  const [sending, setSending] = useState(false);
  const [note, setNote] = useState("");

  const [h, m] = (time || "00:00").split(":").map(Number);
  const start = new Date();
  start.setHours(h || 0, m || 0, 0, 0);
  const end = new Date(start.getTime() + FIVE_H);

  const save = () => api.saveSchedule(p.app, p.name, enabled, time, repeat, message).then(() => onDone(enabled ? "Session start saved" : "Session start turned off")).catch((e) => setErr(errText(e)));
  const sendNow = () => {
    setSending(true);
    setNote("");
    api.sendSessionNow(p.app, p.name, message)
      .then(() => setNote("Sent. A new chat was started and your 5-hour window has begun."))
      .catch((e) => setNote(`Didn't go through: ${errText(e)}`))
      .finally(() => setSending(false));
  };
  const last = schedule?.lastStatus === "sent" && schedule.lastSent ? `Last sent ${new Date(schedule.lastSent * 1000).toLocaleString([], { weekday: "short", hour: "numeric", minute: "2-digit" })}`
    : schedule?.lastStatus === "failed" ? `Last attempt failed: ${schedule.lastNote}` : schedule?.lastStatus === "missed" ? `Last start missed: ${schedule.lastNote}` : "";

  return (
    <Sheet title={`Session start · ${p.name}`} onClose={onClose}>
      <div style={{ marginTop: 10 }}><span style={{ fontSize: 11, fontWeight: 600, letterSpacing: ".06em", padding: "2px 8px", borderRadius: 999, background: "var(--tint-amber-bg)", color: "var(--tint-amber-ink)" }}>BETA</span></div>
      <div style={{ margin: "12px 0 0", padding: 12, borderRadius: 8, background: "var(--raised)", fontSize: 13, lineHeight: 1.5, color: "var(--muted)" }}>
        Tandem sends one short message in a new Claude chat at the time you pick, so your 5-hour window starts when you want it to.
        <ul style={{ margin: "8px 0 0", paddingLeft: 18 }}>
          <li>It uses claude.ai’s unofficial web route, with this profile’s saved sign-in. It can stop working or be blocked at any time.</li>
          <li>This is not the paid Anthropic API. There is no API key and no extra charge: it is the same as typing the message yourself, so it counts toward your plan’s limits.</li>
          <li>The message appears as a normal chat in this account’s chat list, in the Claude app and on claude.ai.</li>
          <li>Automating claude.ai may not be covered by Anthropic’s terms. Use it at your own discretion.</li>
          <li>It only runs while Tandem is open (the tray counts) and this PC is on and online.</li>
        </ul>
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", margin: "18px 0 0" }}>
        <span style={{ fontSize: 14, fontWeight: 500 }}>Start a session automatically</span>
        <button className={enabled ? "switch on" : "switch"} role="switch" aria-checked={enabled} aria-label="Start a session automatically" onClick={() => setEnabled(!enabled)} />
      </div>
      <div style={{ opacity: enabled ? 1 : 0.5, pointerEvents: enabled ? "auto" : "none" }}>
        <label htmlFor="ss-time" style={{ display: "block", fontSize: 13, fontWeight: 500, margin: "16px 0 8px" }}>Start at</label>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <input id="ss-time" type="time" className="field" style={{ width: 130 }} value={time} onChange={(e) => setTime(e.target.value)} />
          <div className="seg" role="radiogroup" aria-label="Repeat">
            {([["daily", "Every day"], ["once", "Once"]] as const).map(([v, t]) => <button key={v} role="radio" aria-checked={repeat === v} className={repeat === v ? "on" : ""} onClick={() => setRepeat(v)}>{t}</button>)}
          </div>
        </div>
        <div style={{ marginTop: 8, fontSize: 13, color: "var(--muted)" }}>The window runs {clock(start)} → {clock(end)}.</div>
        <label htmlFor="ss-msg" style={{ display: "block", fontSize: 13, fontWeight: 500, margin: "16px 0 8px" }}>Message to send</label>
        <input id="ss-msg" className="field" value={message} maxLength={200} onChange={(e) => setMessage(e.target.value)} />
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 16, flexWrap: "wrap" }}>
        <button className="btn" disabled={sending || !message.trim()} onClick={sendNow}>{sending ? "Sending…" : "Send it now to test"}</button>
        <span role="status" style={{ fontSize: 13, color: "var(--muted)" }}>{note}</span>
      </div>
      {last && <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>{last}</div>}
      {err && <div role="alert" style={{ marginTop: 12, fontSize: 13, color: "var(--negative)" }}>{err}</div>}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" onClick={save}>Save</button>
      </div>
    </Sheet>
  );
}

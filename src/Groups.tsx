import { useState } from "react";
import { api, APP_LABEL, errText, initial, type Group, type Member, type Profile, type State } from "./api";
import { Sheet } from "./Sheet";

const same = (a: Member, b: Member) => a.app === b.app && a.name === b.name;

function GroupSheet({ group, profiles, onClose, onDone }: { group: Group | null; profiles: Profile[]; onClose: () => void; onDone: (m: string) => void }) {
  const [name, setName] = useState(group?.name ?? "");
  const [members, setMembers] = useState<Member[]>(group?.members ?? []);
  const [err, setErr] = useState("");
  const toggle = (m: Member) => setMembers((cur) => (cur.some((x) => same(x, m)) ? cur.filter((x) => !same(x, m)) : [...cur, m]));
  const save = () => api.saveGroup(group?.name ?? null, name, members).then(() => onDone(group ? "Group saved" : "Group created")).catch((e) => setErr(errText(e)));
  return (
    <Sheet title={group ? "Edit group" : "New group"} onClose={onClose}>
      <label htmlFor="gname" style={{ display: "block", fontSize: 13, fontWeight: 500, margin: "18px 0 8px" }}>Name</label>
      <input id="gname" className="field" value={name} autoFocus maxLength={40} onChange={(e) => { setName(e.target.value); setErr(""); }} placeholder="Work accounts" />
      <div style={{ fontSize: 13, fontWeight: 500, margin: "18px 0 8px" }}>Profiles</div>
      <div style={{ maxHeight: 240, overflowY: "auto", display: "flex", flexDirection: "column", gap: 2 }}>
        {!profiles.length && <div style={{ fontSize: 13, color: "var(--muted)" }}>Create a profile first.</div>}
        {profiles.map((p) => (
          <label key={p.app + p.name} style={{ display: "flex", alignItems: "center", gap: 10, padding: "6px 4px", fontSize: 14, cursor: "pointer" }}>
            <input type="checkbox" checked={members.some((m) => same(m, p))} onChange={() => toggle(p)} />
            <span className={`av tint-${p.tint}`}>{initial(p.name)}</span>
            <span style={{ flex: 1 }}>{p.name}</span>
            <span style={{ fontSize: 12, color: "var(--muted)" }}>{APP_LABEL[p.app]}</span>
          </label>
        ))}
      </div>
      {err && <div role="alert" style={{ marginTop: 12, fontSize: 13, color: "var(--negative)" }}>{err}</div>}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 24 }}>
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!name.trim()} onClick={save}>Save</button>
      </div>
    </Sheet>
  );
}

export function Groups({ state, groups, reload, toast, afterLaunch, creating, onCreated }: { state: State; groups: Group[]; reload: () => void; toast: (m: string) => void; afterLaunch: () => void; creating: boolean; onCreated: () => void }) {
  const [edit, setEdit] = useState<Group | "new" | null>(creating ? "new" : null);
  const find = (m: Member) => state.profiles.find((p) => same(p, m));
  const close = () => { setEdit(null); onCreated(); };
  const launch = (g: Group) =>
    api.launchGroup(g.name).then((n) => { toast(n ? `Opening ${n} ${n === 1 ? "profile" : "profiles"}` : "Everything in this group is already open"); if (n) afterLaunch(); reload(); }).catch((e) => toast(errText(e)));
  const remove = (g: Group) => api.deleteGroup(g.name).then(() => { toast("Group deleted"); reload(); }).catch((e) => toast(errText(e)));
  return (
    <div style={{ maxWidth: 820, margin: "0 auto" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <h1 style={{ margin: 0, fontSize: 22, fontWeight: 600, letterSpacing: "-0.015em" }}>Groups</h1>
        <button className="btn" onClick={() => setEdit("new")}>New group</button>
      </div>
      <p style={{ margin: "4px 0 20px", fontSize: 14, color: "var(--muted)" }}>Open several profiles at once.</p>
      {!groups.length && <div style={{ padding: "60px 0", textAlign: "center", fontSize: 14, color: "var(--muted)" }}>No groups yet. Create one, or use “Add to group” from a profile’s menu.</div>}
      {groups.map((g) => {
        const live = g.members.map(find).filter((p): p is Profile => !!p);
        return (
          <div key={g.name} style={{ padding: 16, marginBottom: 12, borderRadius: 12, border: "1px solid var(--border)", background: "var(--surface)" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 15, fontWeight: 600 }}>{g.name}</div>
                <div style={{ fontSize: 13, color: "var(--muted)" }}>{live.length} {live.length === 1 ? "profile" : "profiles"}</div>
              </div>
              <button className="btn quiet" onClick={() => setEdit(g)}>Edit</button>
              <button className="btn quiet" onClick={() => remove(g)}>Delete</button>
              <button className="btn primary" disabled={!live.length} onClick={() => launch(g)}>Launch all {live.length || ""}</button>
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 12 }}>
              {live.map((p) => (
                <span key={p.app + p.name} style={{ display: "inline-flex", alignItems: "center", gap: 6, padding: "3px 10px 3px 4px", borderRadius: 999, background: "var(--raised)", fontSize: 13 }}>
                  <span className={`av tint-${p.tint}`} style={{ width: 22, height: 22, fontSize: 11 }}>{initial(p.name)}</span>
                  {p.name}<span style={{ color: "var(--muted)" }}>· {APP_LABEL[p.app]}</span>
                </span>
              ))}
            </div>
          </div>
        );
      })}
      {edit && <GroupSheet group={edit === "new" ? null : edit} profiles={state.profiles} onClose={close} onDone={(m) => { close(); toast(m); reload(); }} />}
    </div>
  );
}

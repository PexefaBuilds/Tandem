"""Writes latest.json for tauri-plugin-updater.
usage: python make_latest_json.py <version> <dir with the signed files> <notes file> <owner/repo>
Needs in <dir>: Tandem_<v>_x64-setup.exe(.sig) and Tandem_<v>_universal.app.tar.gz(.sig)."""
import json, sys, datetime, pathlib

v, d, notes, repo = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3], sys.argv[4]
base = f"https://github.com/{repo}/releases/download/v{v}"
files = {
    "windows-x86_64": f"Tandem_{v}_x64-setup.exe",
    "darwin-aarch64": f"Tandem_{v}_universal.app.tar.gz",
    "darwin-x86_64": f"Tandem_{v}_universal.app.tar.gz",
}
platforms = {k: {"signature": (d / (f + ".sig")).read_text().strip(), "url": f"{base}/{f}"} for k, f in files.items()}
out = {"version": v, "notes": pathlib.Path(notes).read_text(encoding="utf-8").strip(), "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": platforms}
(d / "latest.json").write_text(json.dumps(out, indent=2), encoding="utf-8")

# Tandem — build plan

Each milestone starts with a short plan that the owner approves (see `CLAUDE.md`). Tick the boxes as they pass on a real machine.

## M0 · Repo and shell

Scaffold Tauri 2 + React + TS + Vite with pnpm.
- [x] `tokens.css` and `components.css` ported from `design/Main.dc.html` (light and dark, switched by `prefers-color-scheme` plus a Settings override).
- [x] Window: 1280×800 default, minimum 980×640. Custom title area per platform (traffic lights inset on macOS, caption buttons on Windows).
- [x] Shell matches the design pixel-for-pixel with static data: sidebar, top bar with search centred on the window, sheet.
- [x] Fonts bundled; app icon and tray glyphs exported from `design/Icon.dc.html` (logo C).
- [ ] CI (GitHub Actions): lint, test, and `tauri build` on `windows-latest` and `macos-latest`, uploading artifacts.
- [x] Repo files: README, LICENSE (MIT), SECURITY.md, CONTRIBUTING.md, issue templates.

## M1 · Windows core: desktop profiles

- [ ] Find the Claude and Codex MSIX installs and their versions; the "App not installed" state works.
- [ ] Profiles: create, edit (name and colour), delete with typed confirmation; Main instance rules.
- [ ] Launch and close each profile and the Main instance. Running detection by `--user-data-dir`, ignoring `--type=` children and non-packaged exes.
- [ ] Multi-select launch, tabs, search, empty states.
- [ ] After-launch behaviour (minimize to tray, stay open, quit). Settings persist in `%APPDATA%\Tandem\settings.json`.
- [ ] Keyboard: Tab order, visible focus, Esc closes sheets and menus, Ctrl K focuses search.

## M2 · Sign-in and migration

- [ ] Sign-in flow (waiting, success, failed or timeout) for Claude and Codex, including the Main-instance stash and restore (see `EXISTING-APP.md`).
- [ ] First-launch screen: detect apps; import existing profile folders (any Claude or Codex user-data folder the user points to) without losing their sign-ins (dry run plus confirmation; the original folder is kept until success).

## M3 · Usage

- [ ] Claude usage: cookie decrypt (DPAPI + AES-GCM), org pick, usage endpoint.
- [ ] Codex usage: OAuth token from the profile, `/backend-api/codex/usage`.
- [ ] Parallel refresh on the chosen interval; headroom line; 85% red state.
- [ ] Offline banner, expired-session rows, rate-limit backoff.
- [ ] Near-limit notifications (85%, 100%, weekly 90%) with actions.

## M4 · Tray, groups, shortcuts

- [x] Tray flyout as designed; global shortcut; open at login (starting hidden). (flyout: Claude/Codex rows, groups, footer; terminal rows wait for M5; not hand-tested: icon click, key press, login)
- [x] Groups: create, edit, launch all; "Add to group" picker.
- [x] Desktop shortcuts per profile (`.lnk` that runs `Tandem.exe --launch <app> <profile>`, through single-instance forwarding).

## M5 · Terminal profiles

- [x] Claude Code and Codex CLI detection and versions. (detection untested: neither CLI is on this PC's PATH)
- [x] Launch in Windows Terminal or PowerShell with the env var set, and API keys cleared when "Keep accounts separate" is on.
- [x] Codex `config.toml` written with `cli_auth_credentials_store = "file"`.
- [ ] Sign-in from the sheet (built, not tried with a real login) (opens a terminal running `claude` → `/login`, or `codex login`); detect completion from the credential file.

## M6 · Chat history

- [x] Indexer for Claude Code and Codex session `.jsonl` files (incremental by mtime and size; rescans on opening the page and every minute, no file watcher yet). Reads the default `~/.claude` and `~/.codex` too.
- [x] Import Claude and ChatGPT export zips, assigned to a profile. (tested with a synthetic Claude zip; ChatGPT shape covered by a unit test only)
- [x] FTS5 search, filters, preview, "Resume in terminal", Copy. (Resume not tried: no CLI on this PC)

## M7 · macOS

- [x] App detection in `/Applications`; launching with `open -n -a ... --args --user-data-dir` (compiles and unit-tested on a Mac; **not yet run against the real Claude/Codex Mac apps**).
- [x] Keychain "Safe Storage" key → cookie decrypt; Codex token.
- [x] Menu-bar item, Terminal / iTerm / Ghostty launch, and ⌘, opening Settings from the app menu.
- [ ] Test Claude Code `CLAUDE_CONFIG_DIR` Keychain isolation (a June 2026 bug report said it was broken).

## M8 · Release

- [x] Version 1.0.0; changelog. [ ] Screenshots in the README (needs the owner).
- [ ] Signed builds: Windows through SignPath Foundation (free for open source) or Azure Trusted Signing; macOS Developer ID plus notarization (needs an Apple Developer account).
- [ ] Tauri updater (needs the owner's `pnpm tauri signer generate` key; for now "Check for updates" reads GitHub Releases and opens the page) with signed update manifests on GitHub Releases.
- [x] Release workflow (`.github/workflows/release.yml`, draft release; untested until a tag is pushed) builds `.msi`/`.exe` and `.dmg` with `tauri-action` on tag push.

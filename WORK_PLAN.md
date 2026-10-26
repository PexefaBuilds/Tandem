# Tandem: work plan and progress

Work only in `D:\Claude-Related\ClaudeFiles_Personal\Tandem`. The `G:` shared-drive copy is an old snapshot (no source, no `WORK_PLAN.md`); ignore it. Not a git repo yet; owner said to push to GitHub only when the app is done and tested.

## Done
- **M0** shell, **M1** desktop profiles, **M2** sign-in and import (details in `docs/BUILD-PLAN.md`). Owner's real profiles (FS_Work, FD_Work, Personal(shahid-builds)) are in `%APPDATA%\Tandem`.
- **M3** usage: figures work for FS_Work and FD_Work (plan, 5h, weekly). Usage cache keeps `cf_clearance`; a profile that closes triggers a refresh. Personal plan label (Pro) falls back to org capabilities, unconfirmed.
- **M4** tray flyout (second window `#tray`, per-profile cards with plan, 5h/weekly meters, Launch/Close), hide-to-tray, Ctrl+Shift+Space, open at login (`--hidden`), Groups, desktop `.lnk` shortcuts (`--launch <app> <profile>`), tray icon follows theme.
- **M5** terminal profiles (`terminal.rs`; app keys `cc`/`cx` in `engine::app_by_key`, empty `package_filter`; folder = CLAUDE_CONFIG_DIR / CODEX_HOME; launches `powershell -EncodedCommand` in Windows Terminal or its own console; Codex `config.toml` file store).
- **M6** chat history (`history.rs`: SQLite FTS5 `history.db`; sources = default `~/.claude`, `~/.codex`, terminal profile folders, export zips; `History.tsx`).
- **Beta session start** (`schedule.rs`, `SessionStart.tsx`): sends one message through claude.ai's unofficial web route at a set time. Works ("Send now" produced a "hi" chat). Sheet explains: not the paid API, counts toward limits, may break, ToS risk.
- Extras: startup splash, theme-aware taskbar icon (CSP `connect-src 'self'` was the bug), close never kills Tandem's own ancestors, afterLaunch = minimize (default) | hide | stay | quit.
- Installers rebuilt after M6 (`pnpm tauri build`): `src-tauri/target/release/tandem.exe` and `bundle/{msi,nsis}`.

## Decisions (and why)
- Frontend calls the backend only via `src/api.ts`; name rules live only in Rust (`check_name`), UI validates live through it.
- Heavy Tauri commands are `async`: sync commands run on the main thread and PowerShell calls froze the UI.
- Rename keeps the folder until next launch so running profiles can be edited (`Meta.folder` vs `name`).
- Import excludes regenerable dirs (Cache, vm_bundles, claude-code, logs...) so profiles shrink (2 GB -> 294 MB).
- Claude profile recognised only by its own files (`claude_desktop_config.json`, `ant-did`, oauth key in `config.json`), never generic Chromium files.
- Claude Main-instance data on this PC is at `%LOCALAPPDATA%\Claude-Data\claude_pzs8sxrjxfjjc` (newer layout), not the package folder; `signin::default_slot_dir` checks it first.
- Cookies in the owner's profiles are `v10` (DPAPI + AES-GCM), no `v20`, so M3 cookie decrypt is feasible.
- Terminal tools reuse the desktop-app plumbing: app keys `cc`/`cx` are `App` entries with an empty `package_filter`, so create/rename/delete/groups/shortcuts work unchanged; only launch and detection branch on `is_terminal()`.
- Session start sends through claude.ai's web route (not the paid API) because driving the Claude window by clicks is fragile; it is labelled beta and the sheet states the risks.
- Groups, schedules and chat history are separate files in the data folder (`groups.json`, `schedules.json`, `history.db`) so `profiles.json` stays small and safe to rewrite.
- History is indexed by polling (open page + every minute) instead of a file watcher: simpler, and enough for chats that change slowly.

## Current task
M7 and M8 built (owner said "go"). Mac code lives in `src-tauri/src/platform/macos.rs` plus `cfg(target_os="macos")` branches in engine, terminal, signin, usage, tray, lib. Compiles and `cargo test` passes on the Mac over ssh (synced to `~/ShahidPersonalProjects/Tandem`); nothing was run against the real Mac apps. Version is 1.0.0, `CHANGELOG.md` and `.github/workflows/release.yml` exist, `UPDATE_REPO` = `PexefaBuilds/Tandem`. Private GitHub repo `PexefaBuilds/Tandem` was created by an agent with an initial commit; later edits are uncommitted.

## Next steps
1. Owner on a Mac: launch a profile, check Keychain prompt and usage, menu bar item, Terminal/iTerm/Ghostty, Cmd+, Settings, Dock reopen. Verify `open --env` passes Codex env vars and that Claude Code `CLAUDE_CONFIG_DIR` isolates the Keychain.
2. Owner: `pnpm tauri signer generate` (keep the private key), then wire `tauri-plugin-updater` with the pubkey; add Apple/SignPath secrets for signed builds; README screenshots.
3. Commit and push current changes when the owner agrees; tag `v1.0.0` to run the release workflow; make the repo public when ready.
4. Mac gaps: desktop shortcuts are Windows-only; usage session cache on disk is Windows-only (Mac reads the live cookie file).

## Open questions
- Personal account plan label (Pro): confirm it shows after the capabilities fallback. If not, inspect the organization JSON (never print cookies).
- Claude Code / Codex CLI are not on this PC's PATH, so CLI detection, terminal sign-in and "Resume in terminal" are unverified. Import of the default `~/.claude` as a terminal profile was not built.
- Beta session start: confirm it starts a fresh window right after a reset (owner's session was already live when tested).
- Hotkey has no "Change" button; limit notifications have no action buttons; usage figures are not saved across restarts.

## Untested / hand-test list
- Tray icon click and position, hotkey press, open at login (touches registry; classifier blocks it), desktop shortcut launching a profile.
- Closing and deleting running profiles, rename-on-relaunch, multi-profile launch (a script that kills Claude processes is blocked by the permission classifier; owner may test by hand).
- Real sign-in of a new account, Codex everything (not installed), macOS, a real ChatGPT export, huge `conversations.json` (read fully into memory), CI on GitHub.
- Two Personal cookie files (`Network\Cookies`, `Cookies-journal`) were not copied into the real `%APPDATA%\Tandem` (the profile was open). Copy from `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Tandem\ClaudeProfiles\Personal(shahid-builds)\Network` when it is closed (use a non-virtualized process, see gotchas). Command, run through `Invoke-CimMethod Win32_Process Create`: `robocopy "<Packages path>\Tandem" "C:\Users\_Admin\AppData\Roaming\Tandem" /E /XC /XN /XO` (copy only, never overwrites).

## Gotchas
- **MSIX file virtualization:** anything launched from inside the Claude desktop app (including this Claude Code session's shell) writes `%APPDATA%` files into `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\...`. A Tandem started that way saves profiles there and a normally launched Tandem sees an empty `%APPDATA%\Tandem`. To test the real exe use `TANDEM_DATA_DIR=.devdata` (outside AppData), or start it via `Invoke-CimMethod Win32_Process Create` (not virtualized). Owner's data was copied to the real folder on 2026-10-26 (2 GB); two Personal cookie files may still need copying from the Packages path.
- Shell tool collapses `\\` in heredocs: write Windows paths with `/` in scripts, and use the Edit tool for Rust `'\\'` literals.
- `corepack enable` fails (no admin); pnpm installed via `npm i -g pnpm`; add `/c/Users/_Admin/AppData/Roaming/npm` to PATH.
- TypeScript pinned `~5.9` (typescript-eslint doesn't support 7).
- Platform window config files must repeat the full window entry (arrays are replaced, not merged).
- Dev loop: `pnpm tauri dev` with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`; drive the real window over CDP (scripts used lived in the session scratchpad, not the repo). Vite ignores `.devdata` and `src-tauri` (else copying files reloads the page). Without `TANDEM_DATA_DIR` the dev app uses the owner's REAL `%APPDATA%\Tandem`.
- Never use SendKeys to drive dialogs (keystrokes may land in other windows). This session itself runs inside the imported Personal profile: never kill Claude processes you can't positively identify.
- The owner’s original (pre-import) profile folders are read-only: never modify or delete them. Their location is in the owner’s notes, not here.
- Testing the real exe: launch with `TANDEM_DATA_DIR=<project>/.devdata` plus `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`, then drive it over CDP with node (`WebSocket`, `Runtime.evaluate`, `window.__TAURI_INTERNALS__.invoke(...)`). The app is single-instance: kill `tandem.exe` before rebuilding (the exe is locked) and tell the owner.
- Editing code with Python patches: write the script to a file and run it (inline heredocs with quotes broke in this shell). Delete `.devdata/history.db*` after tests (it holds real chat text).
- Do not set registry values or other persistence to debug (blocked by the classifier); add visible diagnostics in the UI instead.
- `.devdata` holds test data (groups, a schedules file, empty terminal profile folders). Clear `schedules.json` there before any run, or a timed send could fire from the dev copy.

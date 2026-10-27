# Tandem — instructions for Claude Code

Tandem is an open-source desktop app for Windows and macOS that runs several Claude and Codex accounts side by side. Each account lives in its own **profile** with its own sign-in. Tandem launches the official **Claude** and **Codex desktop apps** per profile, plus **terminal profiles** for **Claude Code** and **Codex CLI**. It shows each account's usage limits, launches groups of profiles, lives in the tray or menu bar, and keeps a searchable chat history.

**Tandem is an original, independent implementation.**
- **Write all code fresh** from `docs/SPEC.md`, the design, public documentation (Tauri, Windows, Chromium, Claude Code, Codex CLI) and your own testing.
- **Never copy, translate or port code** from any other multi-instance tool. Don't clone or open such repositories for reference.
- **`docs/EXISTING-APP.md` is private notes** (git-ignored). Use it only for facts about how Windows and the Claude and Codex apps behave: file paths, command-line flags, environment variable names, endpoints, error cases. Don't use it for code structure, wording or naming.
- **No other project is named** in the code, README or About screen.

## How to work in this repo

1. **Plan before you build.** The owner wants a short discussion first. At the start of every milestone in `docs/BUILD-PLAN.md`:
   - write a plan of 10 lines or fewer (what you'll build, files touched, open questions),
   - wait for the owner to say "go",
   - then build.
   Never start the next milestone on your own.
2. **The design is the spec for anything visual.** It lives in `design/` (the source of the Claude Design canvas) and `docs/DESIGN.md` (tokens, components, rules). Match spacing, type, colours and copy exactly. If something isn't designed, follow `docs/DESIGN.md` and say what you assumed.
3. **Behaviour is specified in `docs/SPEC.md`.** Engine details (paths, flags, endpoints) are in `docs/EXISTING-APP.md`. Keep every "gotcha" listed there.
4. **Finish each milestone the same way:**
   - run `pnpm lint`, `pnpm test` and `cargo test`,
   - build the app, and use it on the real OS,
   - tick its acceptance checklist in `docs/BUILD-PLAN.md`,
   - summarise for the owner in plain language.

## Stack

- **Tauri 2.** Rust backend in `src-tauri/`; React + TypeScript + Vite frontend in `src/`; package manager pnpm.
- **Styling:** plain CSS with the design tokens as custom properties (`src/styles/tokens.css`). No UI kit, no Tailwind. Fonts are bundled locally: Instrument Sans and IBM Plex Mono, both under the SIL Open Font License.
- **Tauri plugins:** single-instance, global-shortcut, notification, autostart, updater, opener, dialog. The tray uses Tauri's built-in tray icon.
- **Rust crates:** `serde`, `tokio`, `reqwest` (rustls), `rusqlite` (bundled), `aes-gcm`, `sysinfo`, plus `windows` (DPAPI) and `security-framework` (macOS Keychain) behind `cfg`.
- **Chat history index:** SQLite with FTS5, stored in Tandem's data folder.

## Safety rules (open source; people will trust it with their accounts)

- **Never log, print, send or commit** cookies, OAuth tokens, session keys or API keys. Error messages must not include them either.
- **Secrets never leave the user's machine.** The only network calls are to `claude.ai`, `chatgpt.com` and GitHub (for updates). No telemetry.
- **Usage endpoints are unofficial.** Call them only on the refresh interval (default 5 min) or when the user clicks refresh, and back off on 429.
- **Test against a dev data folder.** Use `TANDEM_DATA_DIR`, never the owner's real profiles, unless they explicitly ask.
- **Deleting is destructive.** Always confirm by typed name, as designed. Kill only processes Tandem started or can positively identify (see the `_is_packaged_exe` gotcha in `docs/EXISTING-APP.md`).
- **Don't use Anthropic or OpenAI logos.** Apps are identified by name and a colour dot only.

## Layout

```
src/              React UI (screens follow design/*.dc.html)
src/styles/       tokens.css, components.css
src-tauri/        Rust: engine (profiles, launch, processes), usage, history, schedule, terminal, tray, shortcut (flat files; M7 adds macOS code in a new platform/macos folder)
design/           design canvas source files (reference only; not shipped)
docs/             SPEC, BUILD-PLAN, DESIGN, EXISTING-APP, OPEN-SOURCE
```

## Current state

Read `WORK_PLAN.md` first. M0 to M8 are built and v1.2.2 is released on GitHub with a signed in-app updater ("Update now") (public repo `PexefaBuilds/Tandem`). macOS is untested on a real Mac; screenshots and code signing wait on the owner; the updater key lives in `~/.tauri/tandem-updater.key` (never print or commit). All commits and pushes must be by PexefaBuilds (see WORK_PLAN gotchas). Mac builds run over `ssh mac`, releases are built and signed locally per "Updater" in `WORK_PLAN.md`. Never print or commit secrets; test against `.devdata` via `TANDEM_DATA_DIR`.

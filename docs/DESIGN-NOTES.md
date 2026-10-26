# Tandem — design notes

Live design canvas (private): https://claude.ai/artifact/4tGg1CLRaYhThLr5NU9SBG

## Direction
- Skills used: taste-skill (app-screen run, style "framed-sheet", seed 1325477876) + apple-design (HIG review lenses).
- Shell: tinted L-frame (sidebar + 48px top bar), white sheet inset 8px, radius 16, no shadow. Only menus, dialogs and the tray flyout cast shadows.
- Type: Instrument Sans (UI) + IBM Plex Mono (paths, env vars). Swapped from the dealt IBM Plex Sans because the brief asks for an Apple-esque feel.
- Colour: neutral chrome; Claude = clay #b4532c / #e38c64 (dark), Codex = blue #2160c4 / #76a5f2 (dark), used only as small identity marks. Usage bars in ink; >=85% used turns red with the words "near limit".
- Per-profile identity: initial + pale tint (graphite, blue, green, amber, rose, teal).
- Signature: "X has the most room right now" — the account with the most 5-hour headroom, with a one-click Launch.
- Light/dark follow the system; Settings also offers Light/Dark overrides (user asked for the options).
- Default after launching: minimize to tray / menu bar.

## Screens on the canvas
1. Main window, light (macOS chrome) — clickable: Claude, Codex, Claude Code, Codex CLI, Groups, Settings; new profile, sign-in, edit, delete, multi-select launch, minimise-to-tray.
2. Main window, dark (Windows chrome).
3. Tray flyout (Windows) and menu bar (macOS).

## Terminal profiles (research, Oct 2026)
- Claude Code: set CLAUDE_CONFIG_DIR per profile -> own login, settings, history (official: code.claude.com/docs/en/iam). macOS Keychain entry is keyed to the dir per docs; a June 2026 bug said otherwise — test on the installed version. A global ANTHROPIC_API_KEY / ANTHROPIC_AUTH_TOKEN overrides every profile, so the launcher must clear them.
- Codex CLI: set CODEX_HOME per profile (auth.json + config.toml). Write cli_auth_credentials_store = "file" into each profile's config.toml to be safe. Login: codex login.
- The desktop Claude app's Code tab uses the desktop profile's own sign-in, so it is already separate per desktop profile (shares ~/.claude settings and skills).
- Opening a terminal: wt.exe can't set env vars, so: wt -w new nt cmd /k "set CLAUDE_CONFIG_DIR=<dir>&& claude"  or  powershell -NoExit -Command "$env:CODEX_HOME='<dir>'; codex". macOS: osascript tell Terminal to do script "export CLAUDE_CONFIG_DIR=<dir>; claude".

## Sample data to replace
Profile names other than FD_Work, all emails (example.com), plans, usage figures, versions and paths are placeholders.

## Update — Oct 24, 2026
- Name chosen: **Tandem**. Mark: two overlapping rounded squares (Claude clay + Codex blue, ink overlap).
- Note: "Tandem" is already a trademark for unrelated products (language app, diabetes devices); fine for personal use, check before public release.
- Added screens: Chat history, First launch, App not installed, Sign-in failed, Offline / expired session, Near-limit notifications, App icon sheet.
- Chat history assumes the safe route: terminal sessions read from each profile folder + desktop chats imported from official data exports.
- Logo chosen: C · Side by side (two offset pills, clay + blue).

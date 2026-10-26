# Tandem — product spec

Visuals: see `design/` and `docs/DESIGN.md`. Engine details: see `docs/EXISTING-APP.md`.

## Concepts

- **App kinds**
  - **Desktop apps:** Claude, Codex. Tandem starts the official app with a profile-specific user-data folder.
  - **Terminal tools:** Claude Code, Codex CLI. Tandem opens a terminal with a profile-specific config folder.
- **Profile:** a named, coloured identity for one account in one app. Each profile owns its own folder.
  - Name rules: 1–40 characters; none of `< > : " / \ | ? *`; unique per app, case-insensitive.
  - Colour is one of six tints: graphite, blue, green, amber, rose, teal. Its initial is the first letter or digit of the name.
- **Main instance:** the app's normal, default login (no custom folder).
  - It is always listed first, marked "Default".
  - It can't be renamed or deleted. It can be launched, closed, signed in again, given a shortcut, and have its folder shown.
- **Group:** a named, ordered set of profiles from any app kinds. "Launch all" opens every member.

## Main window (sidebar + top bar + sheet)

- **Sidebar sections:**
  - Desktop apps: Claude, Codex (each with a profile count).
  - Terminal: Claude Code, Codex CLI.
  - Launch together: Groups, Chat history.
  - Settings, at the bottom.
- **Top bar:**
  - A search box centred on the **window** that filters profiles and groups. Shortcut Ctrl K / ⌘K.
  - "Usage updated N min ago" and a refresh button, on the right.

### Desktop app page (Claude / Codex)

- **Title row:**
  - App name and colour dot.
  - Subline: "Desktop app {version} · N profiles · N running · the Code tab inside each window uses that profile's account".
  - Buttons: "New profile" and the primary launch button. The primary button reads "Select profiles to launch" when disabled, "Launch 1 profile", or "Launch N profiles".
- **Headroom line:** the signed-in profile with the lowest 5-hour usage, written as "X has the most room right now." followed by its detail and a Launch button. If that profile is already running, the button reads "Already running".
- **Tabs:**
  - All, Running and Signed out, with counts.
  - Each tab has its own empty-state text, as designed.
- **Profile row:**
  - Checkbox (disabled when signed out).
  - Avatar, name and email.
  - Plan.
  - Status: Running (green dot), Idle (grey dot) or Signed out (hollow dot).
  - 5-hour and weekly usage: bar, % and reset time. At 85% or more the bar and number turn red and read "near limit".
  - Action button: Launch, Close (when running) or Sign in (when signed out).
  - ⋯ menu:
    - Edit name and colour
    - Sign in again / Sign in
    - Add to group
    - Create or Remove desktop shortcut
    - Show profile folder
    - Delete profile…
- **Launch behaviour:** follows Settings › After launching. Options: Minimize to tray (default), Stay open, Quit.

### Terminal page (Claude Code / Codex CLI)

- **Info strip:**
  - Explains that each profile sets `CLAUDE_CONFIG_DIR` or `CODEX_HOME` to its folder, then runs `claude` or `codex`.
  - Includes a picker for which terminal to open in.
- **Rows:** checkbox, avatar, name, account, folder (mono, shortened), last opened, "Open terminal" and the ⋯ menu.
- **Launch:**
  - Open the chosen terminal with the env var set to the profile's folder.
  - If "Keep accounts separate" is on (the default), also clear `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` and `OPENAI_API_KEY` for that terminal.
  - For Codex, write `cli_auth_credentials_store = "file"` into the profile's `config.toml` when the profile is created.

### Groups

- **Group card:** name, a subline, member chips (avatar, name, app) and "Launch all N".
- **Creating a group:** "New group" lets you pick members.
- **From the ⋯ menu:** "Add to group" opens a small picker of groups.

### Chat history

- **Sources:**
  - Terminal sessions are read live from each terminal profile's folder:
    - Claude Code: `projects/**/<session>.jsonl`
    - Codex: `sessions/**/rollout-*.jsonl`
  - Desktop chats come from importing an official data export zip:
    - Claude export: `conversations.json`
    - ChatGPT export: `conversations.json`
  - The user assigns an imported export to a profile.
- **Layout:**
  - Search field, app filter (All / Claude / Codex / Claude Code / Codex CLI) and a count.
  - A list (380px) beside a preview.
- **Preview:**
  - Title and meta (profile, app, when, message count, source).
  - Messages labelled You / Claude / Codex.
  - Copy button.
  - The primary button reads "Resume in terminal" for terminal sessions (`claude --resume <id>` in the session's cwd, or `codex resume <id>`) and "Open {profile}" for desktop chats.
- **Info strip:** reads "Terminal sessions update on their own. Desktop chats come from each account's data export: X of Y desktop profiles imported", with a "How to export" button.

### Settings (one centred column, max 820px)

- **General:**
  - After launching a profile: Minimize to tray / Stay open / Quit.
  - Show in the system tray / menu bar.
  - Open at login.
  - Quick launch shortcut (default Ctrl+Shift+Space / ⌥⌘Space), with "Change".
- **Appearance:** Theme: System / Light / Dark (default System).
- **Usage limits:**
  - Refresh usage: 1 min / 5 min / 15 min / Manual.
  - Warn me near a limit (notifications at 85% and 100% for running profiles).
- **Terminal profiles:**
  - Open terminal profiles in: Windows Terminal / PowerShell on Windows; Terminal / iTerm / Ghostty on macOS.
  - Keep accounts separate.
- **Apps and storage:** the detected apps with version and path, and the profiles folder with "Show folder".
- **About:** version, "Check for updates", "Source on GitHub".

## Sheets (modal, one at a time, Esc closes, focus starts in the first field)

- **New profile:**
  - App (4-way segmented control), Name (live validation messages as designed), Colour, and "Sign in now" (on by default).
  - "Create profile" is disabled until the form is valid.
- **Sign-in:**
  - Waiting state: indeterminate bar and per-app instructions. Sign-in is detected automatically (see `docs/EXISTING-APP.md`).
  - Timeout: 5 min. If the window closes or the timeout passes, show the failed state: "Sign-in didn't finish", with Try again and Cancel.
  - Success state: "{name} is ready · Signed in as {email} on a {plan} plan", with Done and "Launch {name}".
- **Edit:** name and colour. If the profile is running, the note reads "takes effect next launch". Desktop shortcuts are renamed too.
- **Delete:**
  - Explains what is removed.
  - Requires typing the exact name.
  - If the profile is running, the button reads "Close and delete".

## States

- **First launch:**
  - Shows the apps found (Claude, Codex, Claude Code, Codex CLI, each with version or "Not installed").
  - Offers to import existing profile folders: Claude or Codex user-data folders the user already has, chosen with "Add a folder…" (a folder per profile, or a parent folder holding several). Each import has a checkbox and keeps its sign-in by moving or copying the folder.
  - Buttons: "Start fresh" and "Bring over N profiles".
- **App not installed:** the page explains this, keeps the profiles, and offers "Locate the app…" and "Get {app}".
- **Offline:**
  - A banner reads "You're offline, so usage can't refresh. The figures below are from {time}. Profiles still launch normally." with Try again.
  - The last values stay visible.
- **Session expired (usage):** the row shows "Session expired · Open it once to sign back in and refresh".
- **Notifications:** follow `design/Notification.dc.html`.
  - At 85%: "{profile} is at N% of its 5-hour limit", with Launch {best} and Dismiss.
  - At 100%: "{profile} hit its 5-hour limit", with Launch {best} and "Remind me at reset".
  - Weekly at 90%: no actions.
  - Each threshold notifies at most once per window.

## Tray / menu bar

- **Header:** the mark, "Tandem" and "N signed in".
- **Search field:** filters the list.
- **Sections:**
  - Claude and Codex rows: avatar, name, "N% · 5h", and a running dot. Hovering shows the action.
  - Terminal rows, then Groups with "Launch all".
- **Footer:**
  - Windows: Open Tandem · Settings · Quit.
  - macOS: Settings · Quit · Open Tandem.
- **Shortcut:** the global shortcut toggles this menu.

## Platforms

- **Windows 10 / 11 first.** The desktop apps are MSIX packages (see `docs/EXISTING-APP.md`).
- **macOS 13+ second.**
  - Launch with `open -n -a "<App>" --args --user-data-dir=<dir>`. **Verify that the Claude and Codex Mac apps honour `--user-data-dir`.**
  - Cookie decryption uses the app's "Safe Storage" Keychain item (Chromium on macOS: PBKDF2 + AES-128-CBC). **Verify this.**

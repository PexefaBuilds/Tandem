# Changelog

## 1.2.0
- New: "Update now" in Settings downloads the new version, checks its signature, installs it over the current copy (no uninstall) and restarts Tandem. Windows and macOS. Installs older than 1.2.0 need one manual install of this version first.

## 1.1.0
- New: scheduled session start can now send through Claude Code (`claude -p`) using a signed-in Claude Code profile. It is the default when available, and the web route (beta) stays as the other choice. The Claude Code option is greyed out, with the reason, when the CLI is missing or no profile is signed in.

## 1.0.2
- Fix: the "Download" button after "Check for updates" was blocked and did nothing. It now opens the release page.
- Fix: the built-in app icon (taskbar, pinned shortcut, tray at launch) is now the dark tile, so it is visible on dark taskbars.

## 1.0.1
- Fix: profiles that were open during a scan could not be ticked after you closed them. Rescanning now updates them, and a new Refresh button re-checks which profiles are still open.
- Fix: "Scan this computer" could run for minutes on a slow drive. It now stops after about 14 seconds and the progress estimate matches.

## 1.0.0
First release.
- Run several Claude and Codex desktop accounts side by side, each in its own profile.
- Terminal profiles for Claude Code and Codex CLI.
- Usage limits per profile (5-hour and weekly), tray flyout, groups, desktop shortcuts (Windows), launch at login.
- Searchable chat history across profiles.
- Beta: scheduled session start.
- macOS support: menu-bar item, Terminal / iTerm / Ghostty, Keychain-based usage. Not yet tested on a real Mac.

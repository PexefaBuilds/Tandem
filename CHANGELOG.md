# Changelog

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

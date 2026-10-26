# Tandem

**Your Claude and Codex accounts, in tandem.**

Run several of them side by side.

Tandem keeps each of your Claude and Codex accounts in its own profile with its own sign-in. Launch one or several at once, see how much of each account's usage limit is left, and switch to the account with the most room in one click.

> Status: in development. Windows first; macOS support is built but still untested on a real Mac.

## Features

- **Profiles for every account.** Separate profiles for the Claude and Codex desktop apps, plus terminal profiles for Claude Code and Codex CLI.
- **Usage at a glance.** Each account's 5-hour and weekly limits, reset times, and which account has the most room right now.
- **Launch together.** Pick several profiles, or save them as a group and launch them all with one click.
- **Tray and menu bar.** Launch anything without opening the window, with a global shortcut.
- **Near-limit alerts.** A notification when an account passes 85%, with the best account to switch to.
- **Chat history.** Search past chats across profiles. Terminal sessions are indexed automatically; desktop chats come from your official data export.
- **Looks after itself.** Light and dark mode, opens at login, automatic updates.

## Download

Download the latest release from [Releases](../../releases).

## Privacy

- **Where your data lives.** Profiles are stored on your computer; the location is shown in Settings.
- **What Tandem reads.** Each profile's sign-in cookie or token, only to fetch that account's usage.
- **What it sends.** Only requests to `claude.ai` and `chatgpt.com`, plus GitHub when checking for updates. There is no telemetry.
- **About the usage figures.** They come from the same unofficial endpoints the official apps use, so they may change without notice. You can switch refresh to Manual.

## Build from source

```bash
pnpm install
pnpm tauri dev      # run
pnpm tauri build    # installers
```

You'll need Rust (stable), Node 20+, pnpm, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

## Disclaimer

Tandem is an independent open-source project. It is not affiliated with, endorsed by, or sponsored by Anthropic or OpenAI. Claude and Claude Code are trademarks of Anthropic. Codex and ChatGPT are trademarks of OpenAI.

## License

MIT

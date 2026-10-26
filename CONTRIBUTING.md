# Contributing to Tandem

Thanks for helping. Tandem is MIT-licensed and open to pull requests.

## Setup

Needs Node 22+, pnpm, and a stable Rust toolchain (plus the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS).

```bash
pnpm install
pnpm tauri dev
```

## Before you open a pull request

```bash
pnpm lint
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

## Ground rules

- Visuals follow `docs/DESIGN.md` and the files in `design/`. Behaviour follows `docs/SPEC.md`.
- Never log, print or commit cookies, tokens or API keys. Tandem makes no network calls except to `claude.ai`, `chatgpt.com` and GitHub (updates).
- Test against a throwaway data folder: set `TANDEM_DATA_DIR`, not your real profiles.
- Don't use Anthropic or OpenAI logos.

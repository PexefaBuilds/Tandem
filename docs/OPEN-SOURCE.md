# Releasing Tandem as open source

## Decisions to confirm before the first public release

1. **Name.** "Tandem" is already a trademark for unrelated products, such as a language-exchange app and Tandem Diabetes Care, and other software uses the name.
   - That's a low risk for a free, non-commercial tool, but search USPTO/EUIPO (class 9) before 1.0.
   - Consider a distinct repo and bundle name such as `tandem-launcher` / `app.tandem.launcher`.
2. **Independent implementation.** Tandem is written from scratch, with no code copied or translated from other tools.
   - The idea of running several app instances, and facts like file paths, flags and endpoints, aren't protected.
   - Copied or ported code is protected. If any is ever copied from an MIT project, that project's copyright notice must be kept, so don't copy any.
   - Keep `docs/EXISTING-APP.md` (private notes) out of the public repo. It is in `.gitignore`.
3. **Licence:** MIT (included). It's permissive and common for desktop tools. Put your name in `LICENSE`.

## Required wording (README and About)

> Tandem is an independent open-source project. It is not affiliated with, endorsed by, or sponsored by Anthropic or OpenAI. Claude and Claude Code are trademarks of Anthropic. Codex and ChatGPT are trademarks of OpenAI.

- **Logos:** no Anthropic or OpenAI logos anywhere, including the README and screenshots. Use names and colour dots only.
- **Unofficial endpoints:** state plainly in the README that usage limits come from the same unofficial endpoints the official apps use. They may break, and using them is the user's choice; there's a Manual refresh option.

## Trust and security (what users will look for)

- **Privacy section in the README:**
  - where profiles live,
  - what is read (cookies and tokens, only to fetch usage),
  - what is sent (only to claude.ai and chatgpt.com),
  - no telemetry.
- **SECURITY.md** with a private reporting address.
- **Releases are built by GitHub Actions from tags,** with checksums, so anyone can see what was built.

## Code signing (otherwise SmartScreen and Gatekeeper warn or block)

- **Windows:**
  - **SignPath Foundation** signs open-source projects for free (apply with the public repo).
  - The alternative is Azure Trusted Signing (paid, low monthly cost).
- **macOS:**
  - Developer ID signing plus notarization need an **Apple Developer Program** membership ($99/year).
  - Without it, users must right-click › Open, and some won't.
- **Building macOS without a Mac:** GitHub's macOS runners can build and notarize. Testing still needs a Mac (yours, a friend's, or a cloud Mac).

## Repo checklist

- [ ] README with screenshots, features, download links, privacy, disclaimer, build-from-source instructions
- [ ] LICENSE, SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md
- [ ] Issue templates (bug, feature), PR template
- [ ] CI on every PR; a release workflow on `v*` tags (`tauri-action`); signed updater manifests
- [ ] Topics: claude, codex, multi-account, tauri, desktop

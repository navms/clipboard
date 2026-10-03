# Security Policy

## Supported Versions

Clipboard History is pre-1.0. Fixes land on the latest release only.

| Version | Supported |
|---|---|
| `main` / `develop` | ✅ |
| Latest release | ✅ |
| Older releases | ❌ |

## Reporting a Vulnerability

**Please do not open a public issue for security problems.**

Use GitHub's private reporting flow:

1. Go to the **Security** tab of this repository
2. Click **Report a vulnerability**
3. Fill in as much detail as you can

That opens a private advisory visible only to you and the maintainer.

### What to include

- Affected version (commit SHA or release tag)
- macOS version and whether the Mac is Apple Silicon
- Steps to reproduce
- The impact you believe it has
- Any proof-of-concept code or screenshots

### Response timeline

| Stage | Target |
|---|---|
| Acknowledgement | Within 72 hours |
| Initial assessment | Within 7 days |
| Fix released | Coordinated with you, no arbitrary deadline |

You will be credited in the advisory and in `CHANGELOG.md` unless you prefer
otherwise. Tell us how you would like to be named, or that you would rather
stay anonymous.

## Scope

This application reads the **system clipboard** and stores a local history of
it in `~/Library/Application Support/com.hejin.clipboard/`. That data is
sensitive by nature — it may contain passwords, tokens, and private messages
that passed through the clipboard at some point.

The following are **in scope** for this policy:

- Clipboard history leaving the machine unintentionally (e.g. an updater or
  telemetry path that transmits database contents)
- Missing or bypassable sanitization when displaying captured content
- Code that executes captured clipboard content as if it were markup
- Insecure deserialization in the Tauri IPC surface
- The `asset:` protocol scope escaping `$APPDATA` / `$APPLOCALDATA`
- Weak or missing local encryption of the stored database

The following are **out of scope**:

- The absence of end-to-end encryption. Clipboard history is stored in plain
  SQLite on your own disk, protected only by your OS account permissions. This
  is a documented design decision, not a vulnerability.
- Other local applications reading the same pasteboard. Anything running as your
  user can read the clipboard on macOS; that is an OS-level property.
- Social engineering (e.g. convincing a user to paste a malicious payload
  themselves).

## Security-relevant design notes

Worth knowing when reviewing this codebase:

- **Signing and the Accessibility grant.** The Accessibility (TCC) grant is
  bound to the app's *designated requirement*. An ad-hoc signed bundle has a
  designated requirement consisting of nothing but a CDHash, which changes on
  every rebuild — so the grant silently lapses and the double-tap-Option and
  auto-paste features stop working. `scripts/reinstall.sh` refuses to install
  such a bundle for exactly this reason. Release builds must be signed with a
  stable identity.
- **The `asset:` protocol** is enabled and scoped to
  `$APPDATA/**` + `$APPLOCALDATA/**` (`src-tauri/tauri.conf.json`). Keep it
  scoped; broadening it would expose arbitrary files to the webview.
- **No network calls.** The application makes none. Any PR that introduces
  telemetry or an update check is a security-relevant change that needs to be
  called out in review.
- **The CSP** is deliberately strict: `default-src 'self'`, with
  `img-src` extended for `asset:` and `data:`. Do not relax it to
  `unsafe-eval` — the app does not need it.

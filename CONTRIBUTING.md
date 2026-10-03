# Contributing to Clipboard History

Thanks for considering a contribution. This document covers everything you
need to get from a fresh clone to a merged PR.

## Code of Conduct

By participating, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).
Report unacceptable behavior via the repository's Security Advisory page or by
contacting the maintainer directly.

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| macOS | 11 or newer | Apple Silicon only (arm64) |
| Xcode CLT | any recent | `xcode-select --install` if missing |
| Rust | stable | `rustup toolchain install stable` |
| Node.js | 22 or newer | ships via `fnm` / `nvm` / Homebrew |
| pnpm | 12 or newer | `corepack enable pnpm` |

> **Cargo is often not on your PATH.** A rustup default install puts it in
> `~/.cargo/bin`, which many shell profiles do not export. If `cargo` comes
> back "command not found", prefix your command with:
>
> ```sh
> export PATH="$HOME/.cargo/bin:$PATH"
> ```

## Getting started

```sh
git clone git@github.com:navms/clipboard.git
cd clipboard
pnpm install
pnpm dev:app        # tauri dev — full native app with hot reload
```

`pnpm dev` runs only the Vite frontend on port 1420. It is genuinely useful:
`src/lib/ipc.ts` detects the absence of Tauri and falls back to the fixtures in
`src/lib/mock.ts`, so the entire UI runs in a plain browser with no Rust
toolchain and no real clipboard access. Use it for pure UI work.

## Branch model

This repository follows a git-flow style model:

| Branch | Purpose | Merges into |
|---|---|---|
| `main` | Always releasable. Tags are cut from here. | — |
| `develop` | Integration branch for completed features. | `main` |
| `feature/*` | One feature or fix per branch. | `develop` |
| `fix/*` | Bug fixes branching from `develop`. | `develop` |
| `release/*` | Stabilization for an upcoming release. | `main` + `develop` |
| `hotfix/*` | Urgent fixes from `main`, merged back into both. | `main` + `develop` |

Most contributions only touch `feature/*` and `develop`. If you only have one
change to make, don't bother creating extra branches — just open a PR against
`develop`.

## Commit conventions

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:** `feat` · `fix` · `perf` · `refactor` · `docs` · `test` · `build` ·
`ci` · `chore` · `revert`

**Scopes:** `watcher` · `capture` · `classify` · `store` · `schema` · `paste` ·
`hotkey` · `window` · `tray` · `ui` · `settings` · `deps`

Examples:

```
feat(perf): move PNG encoding off the watcher callback thread

The callback now only snapshots the pasteboard and hands the work to a
dedicated worker, so rapid Cmd+C bursts no longer drop intermediate captures.
```

```
fix(paste): poll for frontmost pid instead of sleeping a fixed 140ms
```

Write the subject in the imperative mood, lowercase, no trailing period. Keep
it under 72 characters. The body is optional for single-line changes, but
required whenever the "why" isn't obvious from the diff.

## Quality gates

CI runs on every push and PR. All three of these must be clean:

```sh
cd src-tauri

cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

```sh
pnpm build          # runs `tsc && vite build`; type errors fail the build
```

`clippy` is run with `-D warnings`, so a single lint is a CI failure. If a lint
is genuinely wrong for a given line, suppress it locally with
`#[allow(clippy::lint_name)]` and a comment explaining why — do not disable
lint groups crate-wide.

`cargo test` currently covers the store and classification layers. New logic in
`src-tauri/src/` should come with tests; a PR that changes behaviour without
tests will get asked for them.

## Pull requests

1. Branch off `develop`.
2. Make sure the quality gates above pass locally — it is faster than waiting
   for CI.
3. Open the PR against `develop` (not `main`).
4. Fill in the PR template. **Describe how you tested the change**; a
   screenshot or a short screen recording is worth a lot for UI work.
5. Expect review. Address comments with new commits rather than force-pushing
   during review, so reviewers can follow the diff incrementally.

### Things reviewers will look for

- **Clipboard-related changes need a stated attribution story.** The watcher
  decides *which application* a capture came from, and that answer is only true
  at the instant the user copies. If your change moves that work off the
  callback, it will break source attribution — say so explicitly.
- **TCC / Accessibility behaviour.** Double-tap-Option and auto-paste depend on
  the Accessibility grant, which is bound to the app's *designated
  requirement*. An ad-hoc signed build loses that grant on every recompile. See
  the note in `scripts/reinstall.sh`.
- **No dead code.** A `#[derive(Serialize)]` that nothing consumes produces no
  compiler warning. If you add a derive, name the code that reads it.

## Adding dependencies

Rust crates go in `src-tauri/Cargo.toml`; JS packages in `package.json`. For
either ecosystem:

- Prefer the standard, actively maintained option over a novel one.
- Add a comment above the dependency explaining *why* it is needed when the
  choice is not obvious.
- Run `cargo udeps` (or a careful manual review) before adding a crate — unused
  dependencies are still shipped inside the binary.
- Note feature flags that exist purely to trim the bundle.

## Releasing

`release.yml` runs on any `v*` tag and publishes a GitHub Release automatically
(`releaseDraft: false`). The build takes about six minutes, most of it a cold
release compile.

**The tag and the version numbers must agree, or the artifact lies.** The
release is named after the git tag, but Tauri names the `.dmg` and `.app.tar.gz`
after `version` in `src-tauri/tauri.conf.json`. Tag `v0.1.1` without bumping that
field and you get a `v0.1.1` release containing a file called
`Clipboard.History_0.1.0_aarch64.dmg`.

```sh
# 1. Bump the version in all three places. `pnpm tauri build` reads the
#    tauri.conf.json one, so that is the one that names the artifact.
#      package.json
#      src-tauri/Cargo.toml
#      src-tauri/tauri.conf.json
#    A plain build refreshes Cargo.lock to match.
cargo check --manifest-path src-tauri/Cargo.toml

# 2. Move whatever sits under [Unreleased] in CHANGELOG.md into a new version
#    section, and add the compare links at the bottom of the file.

# 3. Commit, land on main, wait for CI to go green.

# 4. Tag the commit that actually carries the version bump.
git tag -a v0.1.1 -m "Clipboard History 0.1.1"
git push origin v0.1.1

# 5. When the run finishes, open the release and check the asset names carry
#    the same version as the tag. This is the only place the mismatch above
#    shows up.
```

Two things to know about the output:

- **It is ad-hoc signed and not notarized.** The first launch needs
  right-click → Open, or `xattr -dr com.apple.quarantine`. See
  [SECURITY.md](SECURITY.md).
- **No updater is configured**, so a new release does not appear on anyone's
  machine automatically.

## Reporting bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml). A good
report includes:

- macOS version and whether the Mac is Apple Silicon
- The app version (from the tray menu or the release page)
- What you expected, and what happened instead
- The output of `~/Library/Application Support/com.hejin.clipboard/`
  diagnostics, if relevant
- Whether Accessibility permission is granted

If the app is silently unresponsive after an update, that is almost always the
Accessibility grant lapsing. Re-grant it in System Settings → Privacy & Security
→ Accessibility.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).

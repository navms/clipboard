<!--
Target `develop` by default. Maintainers retarget to `main` for release and
hotfix branches — PRs opened against `main` will be asked to move.
-->

## What this changes

<!-- One or two sentences. What is different after this PR merges? -->

Closes #

## Why

<!--
The problem, not the solution. Link the issue if there is one.
If this fixes a bug, what was the actual cause — the "why" is what stops the
same bug from coming back.
-->

## How it was tested

<!--
Be specific. "Ran it" is not testable by a reviewer.

- [ ] Ran the app (`pnpm dev:app`) and exercised the changed path
- [ ] Added/updated tests in `src-tauri/src/`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `pnpm build`
-->

## Screenshots

<!-- Required for any UI change. Before/after is ideal. -->

## Notes for reviewers

<!--
Anything that deserves extra scrutiny. Examples:

- Does this change what runs on the watcher callback thread? Source
  attribution is only correct at the instant the user copies.
- Does this touch the Accessibility / TCC path? The grant is bound to the app's
  designated requirement.
- New dependency? Why is it needed, and why this one?
- New `#[derive(Serialize)]`? Which code consumes it — a derive with no reader
  compiles silently.
-->

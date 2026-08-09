# Contributing to Env Manager

Thanks for helping make environment-variable work less error-prone. Small, focused changes with clear tests are the easiest to review.

## Before you start

- Use [GitHub Discussions](https://github.com/haechan1103/env_manager/discussions) for open-ended ideas and questions.
- Search [Issues](https://github.com/haechan1103/env_manager/issues) before filing a bug or feature request.
- Open an issue before a large product, security-model, file-format, or architecture change.
- Follow the [Code of Conduct](CODE_OF_CONDUCT.md) and report vulnerabilities through [SECURITY.md](SECURITY.md).

## Local setup

You need Node.js, npm, Rust 1.85+, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/haechan1103/env_manager.git
cd env_manager
npm install
npm run tauri dev
```

## Non-negotiable fixture rule

Never read, copy, commit, log, screenshot, or attach real `.env*` values while developing Env Manager.

- Use the synthetic fixtures under `tests/fixtures` or `crates/env-test-support/fixtures`.
- Use obviously fake values such as `fake_test_token`.
- Redact paths, key names, URLs, and values from diagnostics when they may identify a private project.
- Keep screenshots and GIFs on the built-in synthetic demo project.

## Make a focused change

Keep UI, domain logic, persistence, and platform integration separated. Preserve source env ordering, comments, line endings, unknown syntax, and file permissions whenever the feature does not explicitly change them.

When behavior changes, add the narrowest useful test first. Security-sensitive changes need both an allowed-path test and a denied-path test.

## Verify

Run the checks relevant to your change. The full pre-PR set is:

```bash
npm run check
npm run test:e2e
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Regenerate README media only from the synthetic demo:

```bash
npm run dev -- --host 127.0.0.1
npm run media:readme
```

## Pull requests

- Explain the user problem and the resulting behavior.
- Keep unrelated refactors out of the PR.
- Include tests and screenshots when behavior or UI changes.
- Complete the security and synthetic-data checklist in the PR template.
- Use a clear imperative title, for example `Add linked-value conflict warning`.

By contributing, you agree that your contribution is licensed under the project's [MIT License](LICENSE).

## What changed?

Describe the user problem and the resulting behavior.

## How was it verified?

- [ ] `npm run check`
- [ ] Relevant end-to-end tests
- [ ] `cargo test --workspace` when Rust changed
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` when Rust changed

## Security and data handling

- [ ] I used only synthetic env fixtures and values.
- [ ] This change does not log, screenshot, return, or commit protected values.
- [ ] Allowed and denied paths are tested when access policy behavior changes.
- [ ] Original ordering, comments, line endings, unknown syntax, and permissions remain preserved unless intentionally changed.

## UI evidence

Add before/after screenshots for visible changes. Use the synthetic demo project only.

## Notes for reviewers

Call out migrations, compatibility risks, or follow-up work.

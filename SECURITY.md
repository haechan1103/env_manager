# Security Policy

Env Manager edits environment files that may contain credentials. Please treat every report, screenshot, log, and fixture as potentially sensitive.

## Supported versions

| Version | Security updates |
| --- | --- |
| Latest `0.3.x` release | Supported |
| Earlier releases | Upgrade before reporting |

## Report a vulnerability privately

Use [GitHub's private vulnerability reporting](https://github.com/haechan1103/env_manager/security/advisories/new). Do not open a public issue for a suspected vulnerability.

Include:

- the affected Env Manager version and operating system;
- a minimal reproduction using synthetic values only;
- the expected and observed security boundary;
- relevant logs with project paths, key names, tokens, URLs, and values redacted.

Never attach a real `.env` file, API key, database URL, access token, or private project archive. If a secret was exposed while investigating, rotate it before continuing.

You should receive an acknowledgement within 7 days. We will confirm impact, coordinate a fix and release, and credit the reporter unless anonymity is requested.

## Security boundary

Env Manager is a local file manager, not a secret vault or operating-system sandbox.

- Values stay in the original env files and are processed in memory when required.
- Only projects explicitly registered in the desktop app are accepted by the local broker.
- Normal agent structure responses redact every value.
- `protected` and `unclassified` values cannot be returned through broker value tools.
- A `read-write` value can be returned only through an explicit value operation.
- Claude Code and GitHub Copilot integrations install a direct `.env*` access guard, but tool prompts and hooks are not an OS-level isolation boundary.
- Codex direct-file protection depends on the host's permissions and sandbox configuration.

Use a dedicated production secret manager and appropriate operating-system permissions for high-value credentials.

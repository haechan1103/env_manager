# Security Policy

Kavranta edits environment files that may contain credentials. Please treat every report, screenshot, log, and fixture as potentially sensitive.

## Supported versions

| Version | Security updates |
| --- | --- |
| Latest `0.7.x` release | Supported |
| Earlier releases | Upgrade before reporting |

## Report a vulnerability privately

Use [GitHub's private vulnerability reporting](https://github.com/haechan1103/kavranta/security/advisories/new). Do not open a public issue for a suspected vulnerability.

Include:

- the affected Kavranta version and operating system;
- a minimal reproduction using synthetic values only;
- the expected and observed security boundary;
- relevant logs with project paths, key names, tokens, URLs, and values redacted.

Never attach a real `.env` file, API key, database URL, access token, or private project archive. If a secret was exposed while investigating, rotate it before continuing.

You should receive an acknowledgement within 7 days. We will confirm impact, coordinate a fix and release, and credit the reporter unless anonymity is requested.

## Public repository boundary

The application, Rust crates, broker, agent plugin, build workflows, and security
checks are intentionally public. Public source makes the value-redaction and local
processing claims independently reviewable.

The repository must never contain real env files, signing private keys, Apple
certificates, updater private keys, provider credentials, user project data, or local
application state. Those belong in GitHub Actions secrets, the developer operating
system's credential store, or disposable local build state as appropriate.

`npm run validate:public-boundary` checks tracked files and non-ignored files that
could be committed before builds and releases. It rejects sensitive filenames and
high-confidence credential signatures without printing matching content. GitHub
Secret Scanning and push protection remain an independent server-side layer.

## Security boundary

Kavranta is a local file manager, not a secret vault or operating-system sandbox.

- Values stay in the original env files and are processed in memory when required.
- Only projects explicitly registered in the desktop app are accepted by the local broker.
- Normal agent structure responses redact every value.
- `protected` and `unclassified` values cannot be returned through broker value tools.
- A `read-write` value can be returned only through an explicit value operation.
- A requested local generator can fill one existing occurrence through a five-minute,
  single-use plan and stdin-only Broker command. The agent receives the key and
  affected paths but not the generated value; the selected producer and local Rust
  process necessarily receive it.
- Claude Code and GitHub Copilot integrations install a direct `.env*` access guard, but tool prompts and hooks are not an OS-level isolation boundary.
- Codex direct-file protection depends on the host's permissions and sandbox configuration.
- Provider push runs only after a user or an explicitly requested agent plan starts
  it. Selected values are sent to the displayed GitHub Actions, Cloudflare Workers,
  AWS Secrets Manager, SSM Parameter Store, or locally installed Personal Provider
  Pack target. They are never placed in command arguments or a temporary env file.
- Kavranta does not store provider credentials, fetch remote secret values, or
  continuously synchronize a provider. Authentication remains owned by the official
  CLI, the AWS SDK credential chain, or the locally trusted Pack executable.
- Folder Team Channels write only passphrase-encrypted packages to a folder already
  mounted or synchronized by the operating system. Kavranta does not store the
  passphrase, change folder permissions, mount a NAS, delete shared packages, or
  claim that another device has received them. Anyone with folder read access can
  copy the ciphertext, so send the passphrase through a separate trusted channel.
- Agent tools may list value-free channel and encrypted-package metadata. Package
  publish, passphrase entry, decrypt, conflict review, and apply remain focused
  desktop actions.

Use a dedicated production secret manager and appropriate operating-system permissions for high-value credentials.

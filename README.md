<div align="center">
  <img src="assets/brand/env-manager-logo-v1.png" width="104" alt="Env Manager logo" />
  <h1>Env Manager</h1>
  <p><strong>One place for every <code>.env</code> file your project already uses.</strong></p>
  <p>Edit, link, share, and deploy environment variables—without changing your runtime or pasting protected values into AI chat.</p>
  <p><a href="README.md">English</a> · <a href="README.ko.md">한국어</a></p>
  <p>
    <a href="https://github.com/haechan1103/env_manager/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/haechan1103/env_manager?style=flat-square&color=168463" /></a>
    <a href="https://github.com/haechan1103/env_manager/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/haechan1103/env_manager/ci.yml?branch=main&style=flat-square&label=CI" /></a>
    <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/github/license/haechan1103/env_manager?style=flat-square" /></a>
    <img alt="macOS" src="https://img.shields.io/badge/platform-macOS-111111?style=flat-square&logo=apple" />
    <img alt="Windows" src="https://img.shields.io/badge/platform-Windows-0078D4?style=flat-square&logo=windows" />
  </p>
</div>

![Env Manager demo showing project overview, env editing, AWS deployment, team sharing, and AI tool connections](assets/screenshots/env-manager-demo.gif)

<div align="center">
  <a href="https://github.com/haechan1103/env_manager/releases/latest"><strong>Download for macOS or Windows</strong></a>
  ·
  <a href="#connect-your-ai-coding-agent">Connect an AI agent</a>
  ·
  <a href="SECURITY.md">Security model</a>
</div>

Env Manager is a local-first desktop app. Register a project and it discovers the real `.env`, `.env.local`, `.env.development`, `runtime.env`, Wrangler `.dev.vars`, and nested app env files already used by that project. Values stay in those files; there is no account, hosted vault, or new runtime command to adopt.

## Why use it?

| Keep your current workflow | Update linked values once | Work with AI more safely |
| --- | --- | --- |
| Your existing files and commands remain authoritative. Env Manager preserves paths, comments, ordering, and unrelated formatting. | Explicitly link the same key across two, three, or more files. Edit from any member and save every linked occurrence together. | Codex, Claude Code, and Copilot can inspect structure and perform approved operations through a redacted local broker. Protected values stay out of normal inspection responses. |
| **Share without committing env files** | **Deploy only what you select** | **Catch Git mistakes early** |
| Export all or selected variables as a passphrase-encrypted package, or publish immutable packages through a mounted team folder. | Send selected values to GitHub Actions, Cloudflare Workers, Expo EAS, AWS, or a locally installed CLI Pack without creating a temporary env file. | Detect missing ignore rules, already tracked env files, historical paths, and suspicious public frontend variable names. |

## New in 0.7.0

- **Action Packs:** run a narrowly declared local CLI or fixed HTTPS check with one managed value. Values stay out of the UI, AI conversation, command arguments, logs, and response bodies.
- **Opaque generated values:** let an agent request a five-minute, single-use write plan, then pipe output from `openssl` or another trusted local generator directly into the Broker without exposing the generated value.
- **Broader env discovery and quieter maintenance:** Wrangler `.dev.vars*` files now receive the same discovery, Git-safety, and direct-access guard coverage as `.env*`; app and installed AI integration updates are checked quietly and surfaced only when action is available.

## New in 0.6.5

- **Expo EAS deployment:** send selected values to `development`, `preview`, and `production` through the EAS CLI hidden-value prompt. Values never enter command arguments, temporary files, or Env Manager output.
- **Project-aware checks:** Env Manager detects the nearest EAS project, confirms the signed-in Expo account and project identity, and applies `Sensitive` or `Plain text` visibility per variable.
- **AI-safe EAS operations:** Codex, Claude Code, and Copilot use the same redacted Broker plan and activity trail as the desktop app.

## New in 0.6.4

- **Trusted macOS installation:** Apple Silicon and Intel DMGs are Developer ID signed, and their notarized, stapled apps are verified by the release pipeline before publication.

## New in 0.6.2

- **Folder Team Channels:** use a mounted NAS or existing sync folder to exchange immutable encrypted packages, then review conflicts before applying.
- **AWS deployment:** push to Secrets Manager or SSM `SecureString`, choose an optional KMS key, and compare selected values with redacted `same` / `different` / `unset` results.
- **Remote Runtime checks:** compare a managed file with an allowlisted server target through an age-encrypted SSH verifier; the UI receives equality states, never remote values or hashes.
- **Personal Provider Packs:** add a locally trusted stdin-only CLI integration without waiting for an Env Manager app release.
- **AI provider operations:** supported agents can use the same opaque provider engine and value-free activity log as the desktop app.
- **Cross-project reuse:** copy a protected same-name value between registered projects inside Rust without returning it to the agent or normal UI projection.

## See the workflow

### Organize real env files, not copies

Projects and files can have local display names while their physical paths remain visible and unchanged. Values are masked by default; variable names can be copied, groups can be jumped to quickly, and linked rows show every file affected by Save.

![Env Manager file editor with masked synthetic values, linked files, and group navigation](assets/screenshots/env-manager-editor.png)

### Know what needs attention

The project overview combines missing values, actionable AI-access reviews, parse warnings, Git leak checks, and managed-file navigation without reading values for those checks.

![Env Manager project overview with Git safety and AI access status](assets/screenshots/env-manager-overview.png)

### Share the whole setup—or only the part a teammate needs

<table>
  <tr>
    <td width="50%"><strong>Select files and variables</strong><br />Linked occurrences are selected together. Encrypted export never writes an intermediate plaintext ZIP.</td>
    <td width="50%"><strong>Use a folder your team already has</strong><br />A mounted NAS or sync folder stores ciphertext packages only. Existing folder permissions remain authoritative.</td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/env-manager-encrypted-share.png" alt="Choose individual variables for a passphrase-encrypted Env Manager package" /></td>
    <td><img src="assets/screenshots/env-manager-team-sharing.png" alt="Browse immutable encrypted packages in a Folder Team Channel" /></td>
  </tr>
</table>

Imports add missing variables, preserve receiver-only content, and make differing values explicit. Keep-local is the default; each unlinked conflict is independent, while an existing linked group stays one atomic choice.

![Review target-file mapping and resolve encrypted package conflicts before applying](assets/screenshots/env-manager-import-conflicts.png)

### Push selected values, then verify where verification is possible

<table>
  <tr>
    <td width="50%"><strong>Cloudflare Workers</strong><br />Detect the nearest Wrangler config, verify login/account/Worker access, and send selected Worker Secrets through stdin.</td>
    <td width="50%"><strong>AWS Secrets Manager and SSM</strong><br />Use the local AWS profile or SSO chain, verify account and Region, select KMS, and check equality without displaying values.</td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/env-manager-cloudflare-push.png" alt="Push selected masked variables to a Cloudflare Worker through Wrangler" /></td>
    <td><img src="assets/screenshots/env-manager-aws-compare.png" alt="Push selected variables to AWS and compare deployment values with redacted results" /></td>
  </tr>
</table>

For a server env file outside a project, an administrator can install the fixed Env Manager Verifier and allowlist a target and variable names. Env Manager sends an age-encrypted stdin frame over SSH and runs only the fixed verifier command.

![Compare selected local variables with an allowlisted server Runtime through the encrypted verifier](assets/screenshots/env-manager-runtime-compare.png)

Provider push is always explicit and one-way. GitHub and Cloudflare secret values cannot be read back, so Env Manager does not pretend to verify them. AWS and registered Runtime targets expose a separate comparison operation that returns only equality states. Unselected remote entries are never deleted.

## Supported deployment targets

| Provider | Supported target | How the destination is found |
| --- | --- | --- |
| GitHub Actions | Repository or deployment Environment secrets and configuration variables | Detects the nearest Git worktree and GitHub `origin`, lists accessible repositories and Environments through `gh`, and can explicitly create an Environment. |
| Cloudflare Workers | Worker Secrets for the default Worker or a configured Wrangler environment | Detects the nearest `wrangler.jsonc`, `wrangler.json`, or `wrangler.toml`, then checks the active Wrangler account and Worker access. |
| Expo EAS | Project variables across one or more EAS environments | Detects the nearest `eas.json`, verifies the logged-in project, and sends each value through the EAS CLI hidden prompt instead of `--value`. `EXPO_PUBLIC_` defaults to Sensitive and cannot be EAS Secret. |
| AWS Secrets Manager | One encrypted secret per selected variable | Uses the local AWS profile/SSO credential chain, verifies identity and Region with STS, and supports an optional customer-managed symmetric KMS key. |
| AWS SSM Parameter Store | One `SecureString` parameter per selected variable | Uses the same AWS preflight and optional KMS key, with a configurable path prefix. |
| Remote Runtime | Equality check against one allowlisted server target | Uses a project-shared, value-free target definition and a separately installed fixed SSH Verifier. It does not upload or edit the server file. |
| Personal Provider Pack | A target declared by a locally installed `provider.json` | Runs the declared non-shell executable directly and sends values only through standard input. Packs stay on this computer and can be removed independently. |

Install and sign in to [`gh`](https://cli.github.com/manual/gh_secret_set), [Wrangler](https://developers.cloudflare.com/workers/wrangler/commands/#secret-bulk), or [EAS CLI](https://docs.expo.dev/eas/environment-variables/manage/) before using those providers. AWS uses credentials already configured for the AWS SDK. Review a third-party Provider Pack's manifest and executable before installing it.

## Connect your AI coding agent

One independently versioned local bundle supports **Codex**, **Claude Code**, and **GitHub Copilot / VS Code**. The app detects supported tools and installs or updates their Env Manager connection.

<table>
  <tr>
    <td width="50%"><strong>One connection screen</strong><br />See detection, installed bundle version, update state, and active protection layer per tool.</td>
    <td width="50%"><strong>Value-free activity history</strong><br />See broker structure checks, value-read attempts, mutations, provider checks, and allowed/blocked results. Values and value fragments are never logged.</td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/env-manager-ai-integrations.png" alt="Env Manager connections for Codex, Claude Code, and GitHub Copilot" /></td>
    <td><img src="assets/screenshots/env-manager-ai-activity.png" alt="Value-free AI broker activity with allowed and blocked outcomes" /></td>
  </tr>
</table>

You can also install the integration from a terminal:

<details>
  <summary><strong>Codex</strong></summary>

```bash
codex plugin marketplace add haechan1103/env_manager
codex plugin add env-manager@env-manager
```
</details>

<details>
  <summary><strong>Claude Code</strong></summary>

```bash
claude plugin marketplace add haechan1103/env_manager
claude plugin install env-manager@env-manager
```
</details>

<details>
  <summary><strong>GitHub Copilot CLI / VS Code</strong></summary>

```bash
copilot plugin marketplace add haechan1103/env_manager
copilot plugin install env-manager@env-manager
```
</details>

Register the project in Env Manager first, start a new agent session, and ask naturally:

```text
Inspect this project's env structure without reading values.
Create a Database group and add an empty DATABASE_URL variable.
Link GPT_API_KEY across local and development.
Reuse this registered project's GEMINI_API_KEY here without showing it to me.
Push the selected deployment keys to AWS Secrets Manager under my-service/staging without showing their values.
Push EXPO_PUBLIC_KAKAO_NATIVE_APP_KEY to EAS development, preview, and production as Sensitive without showing its value.
Generate AUTH_SECRET with `openssl rand -base64 32` and save it without showing the value.
```

The integration never registers arbitrary projects. Only projects already registered in the desktop app are accepted by the broker.

### AI access policies

| Policy | Agent access |
| --- | --- |
| `protected` | The agent can see the name and whether a value exists; explicit value reads are blocked. |
| `unclassified` | Treated like `protected` until you choose a policy. |
| `read-write` | A dedicated broker value tool may read or update the value when explicitly invoked. |

Normal structure inspection never returns values, including `read-write` values. Internal operations such as linked saves, cross-project copies, provider pushes, redacted comparisons, and a requested one-time stdin generator do not downgrade this policy.

> Env Manager reduces accidental value exposure, but it is not an operating-system sandbox or a production secret manager. Values remain in the original env files. See [SECURITY.md](SECURITY.md) for the complete boundary.

## Install

Download the installer for your computer from [GitHub Releases](https://github.com/haechan1103/env_manager/releases/latest):

- Windows 10/11 x64 beta (unsigned): `x64-setup.exe`
- Apple Silicon (M1 or newer): `aarch64` DMG
- Intel Mac: `x86_64` DMG

### Windows first launch

The Windows installer is a free **unsigned beta** while the project applies for open-source code signing. Microsoft Defender SmartScreen may show **Windows protected your PC**.

1. Download `x64-setup.exe` only from the [official GitHub Release](https://github.com/haechan1103/env_manager/releases/latest).
2. Open the installer. If SmartScreen appears, select **More info**.
3. Confirm the app name is Env Manager, then select **Run anyway**.

Do not continue if the file came from another site or its details are unexpected. An organization-managed computer may block unsigned applications completely; in that case, contact its administrator instead of bypassing the policy.

### macOS first launch

Starting with `0.6.4`, both macOS DMGs are signed with an Apple Developer ID and contain an app notarized and stapled by Apple before the release is published. macOS may still show the normal confirmation for an app downloaded from the internet; it should identify the developer instead of reporting that Apple cannot verify the app.

If macOS reports an unidentified or unverifiable developer, do not bypass the warning. Confirm that the file came from the [official GitHub Release](https://github.com/haechan1103/env_manager/releases/latest) and report the affected version and Mac architecture.

Env Manager checks one fixed GitHub Releases endpoint for signed app updates. It sends no project path, env metadata, value, or telemetry during the check.

## Develop locally

Requirements: Node.js, npm, Rust 1.85+, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/haechan1103/env_manager.git
cd env_manager
npm install
npm run tauri dev
```

Before opening a pull request:

```bash
npm run check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Use synthetic env fixtures only. Never commit or attach real `.env*` values. Read [CONTRIBUTING.md](CONTRIBUTING.md) before making a change.

## Project status

Env Manager is an early-stage macOS and Windows desktop project. Version `0.7.0` ships signed and notarized macOS builds plus an explicitly unsigned Windows x64 beta, and adds Action Packs, opaque generated-value ingestion, Wrangler `.dev.vars*` support, and background app/AI-integration update checks on top of the existing deploy, share, and redacted-agent workflows. Windows code signing, ARM64, and additional languages remain planned.

## Community

Questions and ideas belong in [GitHub Discussions](https://github.com/haechan1103/env_manager/discussions). Bugs and scoped feature requests belong in [Issues](https://github.com/haechan1103/env_manager/issues).

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md). Security reports must use the private process in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)

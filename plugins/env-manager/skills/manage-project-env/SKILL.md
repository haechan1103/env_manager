---
name: manage-project-env
description: Safely register, inspect, manage, generate, deploy, compare, run trusted local Actions, and coordinate encrypted sharing of project environment variables with Env Manager. Use when an agent must work with env structure, access policy, groups, variables, links, opaque stdin generation, cross-project reuse, Folder Team Channels, deployment providers, value comparison, or stdin-only Provider and Action Packs. Trigger for `.env`, Wrangler `.dev.vars`, Expo/EAS environments, generated secrets such as `openssl rand`, local/development linkage, encrypted team packages, NAS folders, GitHub or Cloudflare secrets, API checks, AWS Secrets Manager, SSM, KMS, or Env Manager.
---

# Manage Project Env

Use only the `env-manager` MCP tools for every supported env-data operation, including
`.env*`, `*.env*`, and Wrangler `.dev.vars*` files, regardless
of whether the host is Codex, Claude Code, GitHub Copilot, or another compatible
agent. Never read, search, print, patch, or write an env file with shell, filesystem,
interpreter, or generic editing tools.

## Workflow

1. Call `inspect_project` with the current project path. If it reports that the
   project is unregistered during a concrete env-management request, call
   `plan_register_current_project`, verify that its displayed root is the current
   workspace intended by the request, and apply it immediately. Then call
   `inspect_project` again.
2. Work only from its redacted structure, presence state, groups, descriptions,
   relationships, and policies.
3. Treat `protected` and `unclassified` as unreadable. Human-known protected-value
   input belongs in the desktop app; a requested local producer may use the opaque
   stdin workflow below without making the value readable.
4. Keep ambiguous unclassified names protected unless the current task explicitly
   requests an access-policy change.
5. Create a plan for every mutation, verify that its paths, names, impact, and risk
   match the current request, then call `apply_plan` immediately. Do not ask for a
   second approval unless the host enforces its own unavoidable tool permission.
6. Ask one concise clarification only when a required factual choice is missing, such
   as the authoritative file for conflicting link values.
7. Report only names, relative paths, groups, link membership, policy, and sanitized
   result codes.

## Register the current project

- Use `plan_register_current_project` only for the Broker's current workspace. The
  tool intentionally accepts no path and must never be used to register some other
  arbitrary folder.
- A concrete request to manage env files in the current workspace authorizes the
  required local registration; do not ask for a second approval. A recommendation
  alone does not authorize registration.
- Registration may create or update the value-free `.env-manager.json` policy
  manifest and the per-computer project registry. It never creates, changes, or
  returns an env file or value.
- If the Broker cannot identify the current workspace safely, ask the user to open
  that project as the agent workspace or register it in the desktop app. Never fall
  back to a path-taking filesystem tool.

## Reuse from another project

- Call `find_reusable_variable_sources` only for a concrete variable name when the
  user asks about reuse or when an empty requested variable makes a same-name source
  recommendation directly useful.
- Present candidate project names and relative files without claiming value equality.
  If more than one candidate remains and the user did not identify one, ask which
  source to use.
- Do not copy from a recommendation alone. Once the user requests a concrete source
  and target, call `plan_copy_variable_from_project`, verify both project identities,
  files, the same key, and every affected target file, then apply the plan.
- If the target occurrence does not exist, create its empty structure first with the
  normal group/add-variable plans, inspect again, and only then create the copy plan.
- Treat the operation as a one-time copy. Never describe it as a cross-project link,
  inheritance, or synchronization. Later edits remain independent.
- The source and target may stay `protected`; never downgrade policy or call
  `read_allowed_value` for this operation.

## Access rules

- Call `read_allowed_value` only when the user explicitly needs the value and the
  manifest policy is already `read-write`.
- Never ask the user to paste a protected value into chat or an MCP argument.
- Change a key to `read-write` only when the current task explicitly requests that
  agent access change. Plan and apply it without another confirmation round trip.
- Prefer `protected` for credential-like names and `unclassified` when uncertain.
- A public/client prefix does not make a credential-looking key safe.

## Generate or pipe a value without reading it

Read [stdin-value-ingest.md](references/stdin-value-ingest.md) when the user asks to
generate a secret locally or pipe a producer's output into an existing managed
variable.

- Create the target with the normal structural tools first when it does not exist.
- Call `plan_stdin_value_write` with the exact managed file, key, and newline policy.
  Verify every linked affected file, then use only the returned opaque plan ID with
  the returned trusted `brokerExecutable` and its fixed `value apply-stdin`
  subcommand.
- Never put the env path, key, produced value, producer output, or a generic command
  in the CLI arguments. Never use this route to read or transform an existing env
  value.
- The five-minute plan is single-use. A failure requires a fresh plan; do not retry
  the old ID or fall back to `plan_set_allowed_value`.
- `protected` and `unclassified` remain unreadable and keep their current policy.

## Push to a deployment provider

- Call `list_deployment_providers` first. Work only with an `available` official or
  locally installed provider returned by the Broker.
- Require a concrete source file, variable names, provider, and destination. Ask one
  concise question when repository, Worker, EAS project/environments, AWS Region/path,
  or Personal Pack target is missing or ambiguous.
- Call `plan_provider_push` with semantic fields only, verify its redacted paths,
  names, destination, and impact, then call `apply_plan` immediately when it matches
  the current request.
- Keep every selection `secret` unless the user explicitly requests a GitHub
  configuration Variable or an Expo EAS visibility. Cloudflare, AWS, and Personal
  Packs accept secret entries only.
- For Expo EAS, pass `easProject` and one or more `easEnvironments`. Prefer
  `sensitive` for public app identifiers unless the user requests `plaintext`.
  Never use `secret` for `EXPO_PUBLIC_` variables: EAS Build must be able to read
  them and the final client bundle exposes them. A public prefix is not permission
  to upload unrelated credential-like keys.
- For AWS, pass an optional profile, an explicit or locally configured Region, an
  optional resource path prefix, and an optional symmetric KMS key alias/ARN. Treat
  KMS as encryption configuration for Secrets Manager or SSM `SecureString`, not as
  a separate env-value destination.
- Never call `gh`, Wrangler, `eas`, AWS CLI, a Personal Pack executable, raw HTTP, or
  shell directly. Protected and unclassified values use the same opaque Broker plan
  and do not require a policy downgrade. The Broker supplies EAS values only through
  the CLI's hidden interactive prompt; do not reproduce `--value` commands.
- Report attempted, succeeded, and failed names only. A completed push is not proof
  of current equality and never implies continuing synchronization.

## Check deployed values without revealing them

- Use this workflow only when the user concretely asks whether selected deployed
  values match. Call `list_deployment_providers`. For AWS, call
  `compare_deployment_values` with the exact provider/profile/Region/path target. For
  a registered Runtime, call `list_runtime_targets`, select its returned source file
  and target ID, then compare with provider `remote-runtime`.
- The comparison tool accepts names and occurrence metadata only. Never pass a
  candidate value, hash, assignment, command, or SDK payload.
- `protected` and `unclassified` variables may use this opaque comparison without a
  policy downgrade. Do not call `read_allowed_value` first.
- Return only `same`, `different`, `unset`, `unverifiable`, or `error` with variable
  and remote-resource names. A last-push receipt is historical activity, not an
  equality result.
- GitHub and Cloudflare secret values are unreadable and therefore unverifiable.
  Never substitute a last-push time, metadata match, or successful CLI exit for a
  live equality claim.
- SSH Runtime checks are allowed only through a target returned by
  `list_runtime_targets`; the Broker encrypts the request and invokes the fixed
  verifier. ECS remains unavailable until its compatible transport is reported.
  Never run SSH, ECS Exec, shell `source`, SHA-256 commands, or arbitrary remote
  scripts yourself.

## Run a locally trusted Action Pack

- Call `list_action_packs` first. Use only an installed `available` Pack returned by
  the Broker; never install, replace, remove, or trust a Pack for the user.
- Require a concrete Pack, managed source file, and one managed variable name for
  every returned binding ID. Ask one concise question if that mapping is ambiguous.
- Call `plan_action` with the Pack ID, file, and binding-ID-to-variable-name map.
  Verify the redacted Pack, file, names, and target summary, then call `apply_plan`
  immediately when they match the user's request.
- Never call the Pack executable, curl, an HTTP client, shell, or generic command
  tool directly. Never pass a value, assignment, request body, header value, command,
  or argument fragment to Broker tools.
- Report only success, stable result code, optional HTTP status, duration, and exit
  code. A successful status does not authorize reading or describing a response body.
- Protected and unclassified values can run through this opaque path. Do not call
  `read_allowed_value` or downgrade policy first.
- The installed CLI or fixed API receives the value by design. Do not describe the
  action as local-only, encryption, equality verification, or proof that the target
  handles the value safely.

## Encrypted team folders

- Call `list_team_channels` when the user asks which Folder Team Channels are
  connected, whether one is readable, or which encrypted packages are available for
  the current registered project.
- Return only channel names/IDs, capability state, package IDs, sizes, and timestamps.
  Never infer package contents or claim that one package is the canonical latest.
- Never ask for or accept a sharing passphrase in chat or any Broker argument. The
  passphrase-based publish, decrypt, conflict review, and apply flows are focused
  actions in the Env Manager desktop app.
- If the requested action needs package values, tell the user exactly which channel
  and package to open in **Team sharing**. Do not read the shared folder with shell or
  generic filesystem tools, even when its path is known outside the Broker.
- Treat `readable: false` as an existing mount/sync/permission problem. Do not change
  ACLs, mount storage, delete packages, or attempt a vendor-specific workaround.
- The Broker leaves publish capability unchecked to keep inspection read-only. Tell
  the user to open **Team sharing** for the desktop app's focused write-capability
  probe. Do not promise that Env Manager can bypass the storage provider's
  permissions.

## Author a Personal Provider Pack

Read [personal-provider-packs.md](references/personal-provider-packs.md) when the user
asks to support an unsupported CLI or create a reusable local integration.

- Verify the current official CLI documentation before choosing arguments or a
  supported semantic-version range.
- Create only the Pack source files requested by the user. Do not install, replace,
  trust, or execute the Pack on the user's behalf.
- Keep values out of the manifest and arguments. A Pack may interpolate `key` and one
  declared target placeholder; the selected value always arrives through stdin.
- Tell the user to install the generated `provider.json` from Env Manager. Once
  installed, future provider requests use the semantic Broker workflow above rather
  than reproducing CLI syntax in chat.

## Author an Action Pack

Read [action-packs.md](references/action-packs.md) when the user asks to create a
reusable non-provider CLI task, fixed API check, or Action Pack.

- Verify the intended executable/API and CLI version contract from official
  documentation before authoring.
- Create source files only. Never install, replace, trust, or execute the generated
  Pack on the user's behalf.
- Keep endpoints fixed and keep values out of manifests, URLs, arguments, examples,
  and tests. Use unmistakably fake canaries in test fixtures.
- Tell the user to inspect the target and install `action.json` from Env Manager.
  Future executions must use `list_action_packs → plan_action → apply_plan`.

## Structural changes

- Use `plan_create_env_file` when the requested env file does not exist. It creates
  only an empty supported env-data file inside an existing registered-project
  directory and never overwrites a path. This includes `.env*`, `*.env*`, and
  Wrangler's `.dev.vars` or `.dev.vars.<environment>` names. Apply it immediately,
  inspect again, then add empty variables with `plan_add_variable`.
- Use `plan_create_group` to create an empty explicit group and
  `plan_rename_group` to rename exactly one existing group. `기타` is the virtual
  ungrouped area and cannot be created or renamed as an explicit group.
- Use `plan_add_variable` to add a variable with an empty value. This tool has no
  value argument. Tell the user to enter a protected value in the desktop app.
- Use `plan_move_variable` to move a variable and its contiguous description to an
  existing group. To move into a new group, create and apply that group first, then
  make a separate move plan.
- Use `plan_update_description` for ordinary comment lines attached to one variable.
  Do not encode group markers or other `# @` directives as descriptions.
- Use `plan_migration` for strong visual headings such as `# === GPT ===`,
  `# ** GPT **`, or `# [GPT]`. Ordinary comments remain descriptions or preserved
  notes.
- Link occurrences only by explicit key and file membership. Never infer a link from
  matching names.
- For conflicting non-empty link members, require the user to choose the source file.
- Support two or more peers; do not model whole-file inheritance.
- Detach preserves the occurrence's current value.
- If duplicate group names make a target ambiguous, stop and ask the user to rename
  or resolve them; never choose the first matching marker.

Read [tool-safety.md](references/tool-safety.md) when a tool is rejected, a plan
expires, or an operation involves multiple files.

# Tool safety and recovery

## Tool sequence

| Intent | Required sequence |
| --- | --- |
| Inspect | `inspect_project` |
| Replace allowed value | `plan_set_allowed_value` → `apply_plan` |
| Write a producer value without reading it | `plan_stdin_value_write` → `env-manager-broker value apply-stdin --plan …` |
| Create empty env file | `plan_create_env_file` → `apply_plan` → `inspect_project` |
| Add empty variable | `plan_add_variable` → `apply_plan` |
| Create group | `plan_create_group` → `apply_plan` |
| Rename group | `plan_rename_group` → `apply_plan` |
| Move variable | `plan_move_variable` → `apply_plan` |
| Update description | `plan_update_description` → `apply_plan` |
| Link N occurrences | `plan_link` → `apply_plan` |
| Detach one member | `plan_detach` → `apply_plan` |
| Change explicitly requested access | `plan_classification` → `apply_plan` |
| Normalize groups | `plan_migration` → `apply_plan` |
| Find same-name sources in other projects | `find_reusable_variable_sources` |
| One-time opaque project copy | `find_reusable_variable_sources` → `plan_copy_variable_from_project` → `apply_plan` |
| Inspect available deployment providers | `list_deployment_providers` |
| Opaque provider push | `list_deployment_providers` → `plan_provider_push` → `apply_plan` |
| Redacted provider comparison | `list_deployment_providers` → `compare_deployment_values` |
| Run installed Action Pack | `list_action_packs` → `plan_action` → `apply_plan` |

## Failures

- `UNREGISTERED_PROJECT`: ask the user to register the folder in the desktop app.
- Invalid new-file path: use only a supported `.env*`, `*.env*`, `.dev.vars`, or
  `.dev.vars.<environment>` name below an existing project directory. Never overwrite
  an existing file, create an example variant, or fall back to generic filesystem
  tools.
- `CODEX_ACCESS_BLOCKED`: this stable protocol code means agent access is blocked.
  Keep the value protected; direct the user to desktop input or ask whether they want
  to review classification.
- `FILE_CHANGED_EXTERNALLY`: inspect again, create a new plan, and never force apply.
- `PLAN_EXPIRED`: create a fresh plan and present it again.
- stdin plan failure: read [stdin-value-ingest.md](stdin-value-ingest.md), create a
  fresh plan, and never retry a consumed plan or fall back to direct env access.
- `LINK_VALUE_CONFLICT`: ask which selected file is authoritative; never choose by
  filename, ordering, or guessed environment.
- Invalid or ambiguous group target: inspect again. `기타` means the ungrouped area;
  create a new explicit group before moving to it. Never select among duplicate
  group names by position.
- Missing or ambiguous project-copy source: search again by the exact key and ask the
  user to select a candidate project/file. Never inspect the value, choose by file
  environment, or turn the copy into a continuing link.
- Missing provider/target or unavailable Adapter: list providers again and ask for
  the missing semantic destination. Never fall back to a shell, CLI command, raw HTTP,
  or value read.
- AWS authentication, Region, permission, or KMS failure: report the stable failure
  and ask the user to fix their local AWS Profile/SSO session or target choice. Never
  request credentials or a secret value in chat.
- `EAS_CONFIG_NOT_FOUND`: ask the user to connect the source env file to an app
  directory containing `eas.json`. Never guess another workspace package.
- `EAS_ACCESS_UNAVAILABLE`: ask the user to run the official EAS login flow locally
  or obtain access to the detected project. Never request an Expo token in chat.
- `EAS_PROJECT_MISMATCH`: report the requested project name and ask the user to fix
  the local EAS project/account selection. Never push to the project that happened to
  be logged in.
- `EAS_PUBLIC_SECRET_UNSUPPORTED`: use `sensitive` or, only when explicitly requested,
  `plaintext` for `EXPO_PUBLIC_`; do not retry as `secret`.
- Provider comparison `unverifiable`: explain that the target does not return values
  or lacks the fixed remote verifier. For Runtime comparison, list registered targets
  and use only the returned target ID and source file. Never infer equality from a
  prior push receipt and never fall back to a shell/hash/SSH recipe.
- Missing, invalid, unavailable, or failed Action Pack: list Packs again and report
  the stable code. Ask the user to inspect/install a compatible Pack in the desktop
  app when needed. Never fall back to its executable, curl, raw HTTP, shell, or a
  direct value read.

## Output allowlist

Return project IDs, relative paths, the trusted Broker executable path supplied by
an opaque stdin plan, variable names, descriptions, group names,
presence (`empty` or `present`), policy, link IDs/members, plan IDs, risks, and
sanitized result codes, Action success, optional HTTP status, optional duration, and
optional CLI exit code. Do not return assignment lines, values, value fragments,
masked prefixes/suffixes, hashes, response bodies, CLI output, or MCP argument echoes.

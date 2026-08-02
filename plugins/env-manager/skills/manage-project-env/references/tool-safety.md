# Tool safety and recovery

## Tool sequence

| Intent | Required sequence |
| --- | --- |
| Inspect | `inspect_project` |
| Replace allowed value | `plan_set_allowed_value` → approval → `apply_plan` |
| Add empty variable | `plan_add_variable` → approval → `apply_plan` |
| Create group | `plan_create_group` → approval → `apply_plan` |
| Rename group | `plan_rename_group` → approval → `apply_plan` |
| Move variable | `plan_move_variable` → approval → `apply_plan` |
| Update description | `plan_update_description` → approval → `apply_plan` |
| Link N occurrences | `plan_link` → approval → `apply_plan` |
| Detach one member | `plan_detach` → approval → `apply_plan` |
| Change access | `plan_classification` → approval → `apply_plan` |
| Normalize groups | `plan_migration` → approval → `apply_plan` |

## Failures

- `UNREGISTERED_PROJECT`: ask the user to register the folder in the desktop app.
- `CODEX_ACCESS_BLOCKED`: keep the value protected; direct the user to desktop input
  or ask whether they want to review classification.
- `PROTECTION_DOWNGRADE_REQUIRES_CONFIRMATION`: explain the named key's new exposure,
  then wait for an explicit answer.
- `FILE_CHANGED_EXTERNALLY`: inspect again, create a new plan, and never force apply.
- `PLAN_EXPIRED`: create a fresh plan and present it again.
- `LINK_VALUE_CONFLICT`: ask which selected file is authoritative; never choose by
  filename, ordering, or guessed environment.
- Invalid or ambiguous group target: inspect again. `기타` means the ungrouped area;
  create a new explicit group before moving to it. Never select among duplicate
  group names by position.

## Output allowlist

Return project IDs, relative paths, variable names, descriptions, group names,
presence (`empty` or `present`), policy, link IDs/members, plan IDs, risks, and
sanitized result codes. Do not return assignment lines, values, value fragments,
masked prefixes/suffixes, hashes, or MCP argument echoes.

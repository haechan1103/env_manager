---
name: manage-project-env
description: Safely inspect and manage environment-variable files in projects registered with Env Manager. Use when a user asks Codex to inspect env structure, create a new env file, classify variable access, create or rename groups, add or move variables, update descriptions, replace an allowed value, normalize existing group comments, link the same key across two or more env files, or detach a link. Trigger for natural requests mentioning `.env`, env groups, environment variables, local/development linkage, or Env Manager.
---

# Manage Project Env

Use only the `env-manager` MCP tools for every `.env` or `.env.*` operation. Never
read, search, print, patch, or write an env file with shell, filesystem, interpreter,
or generic editing tools.

## Workflow

1. Call `inspect_project` with the current registered project path.
2. Work only from its redacted structure, presence state, groups, descriptions,
   relationships, and policies.
3. Treat `protected` and `unclassified` as unreadable. Direct protected-value input
   and replacement belong in the desktop app.
4. Keep ambiguous unclassified names protected unless the current task explicitly
   requests an access-policy change.
5. Create a plan for every mutation, verify that its paths, names, impact, and risk
   match the current request, then call `apply_plan` immediately. Do not ask for a
   second approval.
6. Ask one concise clarification only when a required factual choice is missing, such
   as the authoritative file for conflicting link values.
7. Report only names, relative paths, groups, link membership, policy, and sanitized
   result codes.

## Access rules

- Call `read_allowed_value` only when the user explicitly needs the value and the
  manifest policy is already `read-write`.
- Never ask the user to paste a protected value into chat or an MCP argument.
- Change a key to `read-write` only when the current task explicitly requests that
  access change. Plan and apply it without another confirmation round trip.
- Prefer `protected` for credential-like names and `unclassified` when uncertain.
- A public/client prefix does not make a credential-looking key safe.

## Structural changes

- Use `plan_create_env_file` when the requested env file does not exist. It creates
  only an empty `.env` or `.env.*` file inside an existing registered-project
  directory and never overwrites a path. Apply it immediately, inspect again, then
  add empty variables with `plan_add_variable`.
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

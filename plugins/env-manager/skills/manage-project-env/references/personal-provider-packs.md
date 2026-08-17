# Personal Provider Pack contract

Use this reference only when authoring a local Pack for a CLI that Env Manager does
not support officially. A Pack is trusted local configuration, not a security review
or an official integration.

## Required `provider.json`

```json
{
  "schemaVersion": 1,
  "id": "local.example.deploy",
  "displayName": "Example Deploy",
  "description": "Push one selected secret through the Example CLI.",
  "version": "1.0.0",
  "providerProtocolVersion": "0.2.0",
  "valueTransport": "stdin",
  "target": {
    "label": "Application",
    "placeholder": "target"
  },
  "cli": {
    "executableCandidates": ["example-cli"],
    "versionArgs": ["--version"],
    "profiles": [
      {
        "id": "example-v1",
        "versionRequirement": ">=1.0.0,<2.0.0",
        "pushArgs": ["secret", "set", "{key}", "--app", "{target}"]
      }
    ]
  }
}
```

The example arguments are illustrative fake syntax. Replace them only after checking
the target CLI's current official documentation.

## Closed rules

- Use a namespaced ID beginning with `local.` and at least three dot-separated
  kebab-case segments.
- Use semantic versions for the Pack and each Profile requirement.
- Declare one to eight executable candidates, version arguments, and Profiles within
  the app's size limits.
- Use `{key}` exactly once or where required by the CLI. If `target` is present, use
  its declared placeholder in the Profile arguments. Omit both when no target exists.
- Never use `{value}`, another placeholder, an environment variable, temporary file,
  shell pipeline, redirection, command substitution, or quoting trick.
- Do not select shells or general interpreters such as sh, bash, zsh, cmd,
  PowerShell, Python, Node, Bun, Deno, Ruby, or Perl. Windows `.cmd` and `.bat`
  launchers are not supported for Personal Packs.
- Prefer an official SDK adapter when the CLI cannot receive exactly one secret on
  stdin or requires complex credential handling.

## Handoff

Return the Pack path, ID, version, verified CLI version range, and semantic target
meaning. Do not run it. Ask the user to open Env Manager, choose **Push variables →
Add CLI Pack**, and select `provider.json`. Installation is the user's local trust
decision and is never shared automatically through a project checkout.

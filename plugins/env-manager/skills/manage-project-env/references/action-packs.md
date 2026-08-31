# Action Pack authoring

Use an Action Pack only for one bounded, locally trusted CLI or HTTP operation. It is
not a generic shell or HTTP client. The user installs `action.json` in Env Manager;
an agent never installs or executes source files directly.

## HTTP

```json
{
  "schemaVersion": 1,
  "id": "local.example.api-check",
  "displayName": "Example API check",
  "description": "Checks one fixed endpoint",
  "packVersion": "1.0.0",
  "actionProtocolVersion": "0.1.0",
  "type": "http",
  "method": "GET",
  "url": "https://api.example.com/health",
  "secretBindings": {
    "Authorization": {
      "source": "header",
      "format": "Bearer {value}"
    }
  },
  "resultPolicy": {
    "status": true,
    "duration": true,
    "body": false,
    "successStatusCodes": [200]
  },
  "timeoutSeconds": 15
}
```

- Use a fixed HTTPS URL. Loopback HTTP is for local development only.
- Do not use URL placeholders, credentials, query strings, fragments, request
  bodies, or response bodies.
- Define 1–16 header bindings. Each format contains exactly one `{value}`.
- `body` must be `false`; redirects and body output are always unavailable.

## CLI

```json
{
  "schemaVersion": 1,
  "id": "local.example.cli-upload",
  "displayName": "Example CLI upload",
  "description": "Sends one value through standard input",
  "packVersion": "1.0.0",
  "actionProtocolVersion": "0.1.0",
  "type": "cli",
  "executableCandidates": ["example-cli"],
  "versionArgs": ["--version"],
  "profiles": [{
    "id": "example-v2-v3",
    "versionRequirement": ">=2,<4",
    "arguments": ["secret", "set", "{variableName}"]
  }],
  "secretBinding": "value",
  "secretTransport": "stdin",
  "resultPolicy": {
    "success": true,
    "exitCode": true,
    "duration": true
  },
  "timeoutSeconds": 30
}
```

- Choose the narrowest semantic-version range verified against official docs.
- Use direct executable candidates only. Shells, interpreters, `.cmd`, and `.bat`
  launchers are rejected.
- Each Profile contains exactly one `{variableName}` across its argument tokens.
- Protocol v1 has one secret binding and `stdin` is the only value transport.
- Never add `{value}` to arguments, environment variables, paths, or files.
- stdout/stderr are discarded. Results contain only status metadata.

Use a Personal Provider Pack instead when the operation is a normal per-variable
deployment push with provider targets. Use a compiled official adapter when the
service requires response parsing, OAuth handling, request bodies, value-bearing
arguments, temporary files, multi-step transactions, or provider-specific retries.

# Opaque stdin value ingest

Use this workflow only when the user has requested that a local producer create the
new value for one existing managed occurrence. The producer and Env Manager receive
the value; the agent, MCP arguments, process arguments, normal output, and audit log
must not.

## Required sequence

1. Inspect the project and resolve one exact file and key. If the occurrence is
   missing, create the empty structure through the normal Broker plans first.
2. Call `plan_stdin_value_write`. Set `trimFinalNewline: true` only when exactly one
   producer-added final LF, or CRLF, should be removed.
3. Verify the returned key and every affected linked file. The plan expires after
   five minutes and can be claimed only once.
4. Run a fixed pipeline containing only the opaque plan ID:

   ```sh
   set -o pipefail
   openssl rand -base64 32 | "<BROKER_EXECUTABLE>" value apply-stdin \
     --plan <OPAQUE_PLAN_ID> --trim-final-newline
   ```

   Omit `--trim-final-newline` when the plan has `trimFinalNewline: false`.
5. Treat a nonzero pipeline status or a result other than `OK` as failure. Inspect the
   project again to confirm presence and linked impact without reading the value.

For a producer that could emit partial output before returning failure, gate its
output in shell memory so the Broker receives either the complete value or empty
stdin, which it rejects:

```sh
set -o pipefail
{ env_manager_generated="$(producer)" && printf '%s' "$env_manager_generated"; } |
  "<BROKER_EXECUTABLE>" value apply-stdin --plan <OPAQUE_PLAN_ID>
```

Do not print, trace, inspect, or interpolate `env_manager_generated` into arguments.
Use `brokerExecutable` exactly as returned by the plan; do not search for or
substitute another executable.
Do not enable `set -x`. Never replace `producer` with a command that reads an env
file, clipboard, keychain, credential store, or remote secret unless a separately
approved typed capability owns that source.

## Failure rules

- `PLAN_EXPIRED`: create a new plan. The previous plan may be expired, already used,
  or consumed by a failed attempt.
- `FILE_CHANGED_EXTERNALLY`: inspect again and create a new plan; never force it.
- `STDIN_NORMALIZATION_MISMATCH`: recreate or invoke the plan with the exact newline
  option shown in its projection.
- `STDIN_VALUE_EMPTY`, `STDIN_VALUE_TOO_LARGE`, `STDIN_VALUE_INVALID`, or
  `STDIN_VALUE_INVALID_UTF8`: fix the producer and create a fresh plan.
- Never echo producer stdout/stderr into the response. Report only the stable result
  code, key, and affected relative files.

This reduces accidental exposure to the agent. It does not isolate the value from
the selected producer, Env Manager's Rust process, malware, a debugger, or another
process with the same operating-system user's authority.

## PowerShell

In PowerShell, gate the producer before invoking the returned executable and check
both native exit codes:

```powershell
$broker = "<BROKER_EXECUTABLE>"
$envManagerGenerated = & openssl rand -base64 32
if ($LASTEXITCODE -ne 0) { throw "VALUE_PRODUCER_FAILED" }
$envManagerGenerated | & $broker value apply-stdin --plan <OPAQUE_PLAN_ID> --trim-final-newline
if ($LASTEXITCODE -ne 0) { throw "ENV_MANAGER_STDIN_APPLY_FAILED" }
Remove-Variable envManagerGenerated
```

Do not print `$envManagerGenerated`, put it in a process argument, or retain it after
the operation.

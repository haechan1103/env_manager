use super::super::*;

pub fn tool_definitions() -> Value {
    json!([
        tool(
            "plan_register_current_project",
            "Plan local registration of the broker's current Git worktree or recognized project workspace. Takes no path, never returns env values, and never changes env files.",
            json!({
                "type": "object", "properties": {}, "additionalProperties": false
            })
        ),
        tool(
            "inspect_project",
            "Return redacted env structure and value presence for a registered project.",
            json!({
                "type": "object", "properties": { "projectPath": { "type": "string" } }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "find_reusable_variable_sources",
            "Find same-name variables with present values in other registered projects. Returns project and file metadata only, never values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" }
                }, "required": ["projectPath", "key"], "additionalProperties": false
            })
        ),
        tool(
            "read_allowed_value",
            "Explicitly read one value only when its policy is read-write.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "key": { "type": "string" }
                }, "required": ["projectPath", "file", "key"], "additionalProperties": false
            })
        ),
        tool(
            "plan_set_allowed_value",
            "Create a redacted plan to replace a read-write value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "key": { "type": "string" }, "newValue": { "type": "string" }
                }, "required": ["projectPath", "file", "key", "newValue"], "additionalProperties": false
            })
        ),
        tool(
            "plan_stdin_value_write",
            "Create a five-minute, single-use, value-free plan for writing one managed occurrence from the returned trusted Broker executable's stdin. Protected and unclassified values remain unreadable, and linked members are included in the impact.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" },
                    "file": { "type": "string" },
                    "key": { "type": "string" },
                    "trimFinalNewline": { "type": "boolean", "default": false }
                },
                "required": ["projectPath", "file", "key"],
                "additionalProperties": false
            })
        ),
        tool(
            "plan_create_env_file",
            "Plan creating one empty supported env file (.env*, *.env*, or Wrangler .dev.vars*) inside an existing registered-project directory. Existing files and example variants are rejected.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "file"], "additionalProperties": false
            })
        ),
        tool(
            "plan_add_variable",
            "Plan adding a variable with an empty value. This tool never accepts or returns a value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" }, "group": { "type": "string" },
                    "description": { "type": "array", "items": { "type": "string" } }
                }, "required": ["projectPath", "file", "key", "group"], "additionalProperties": false
            })
        ),
        tool(
            "plan_create_group",
            "Plan adding one explicit # @group marker without reading or changing values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "name": { "type": "string" }
                }, "required": ["projectPath", "file", "name"], "additionalProperties": false
            })
        ),
        tool(
            "plan_rename_group",
            "Plan renaming one unambiguous explicit group marker.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "currentName": { "type": "string" }, "newName": { "type": "string" }
                }, "required": ["projectPath", "file", "currentName", "newName"], "additionalProperties": false
            })
        ),
        tool(
            "plan_move_variable",
            "Plan moving an existing variable and its attached description to an existing group.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" }, "targetGroup": { "type": "string" }
                }, "required": ["projectPath", "file", "key", "targetGroup"], "additionalProperties": false
            })
        ),
        tool(
            "plan_update_description",
            "Plan replacing the ordinary comment lines attached to one variable without reading its value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" },
                    "lines": { "type": "array", "items": { "type": "string" } }
                }, "required": ["projectPath", "file", "key", "lines"], "additionalProperties": false
            })
        ),
        tool(
            "plan_link",
            "Plan an N-way peer link without returning any values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" },
                    "files": { "type": "array", "items": { "type": "string" }, "minItems": 2 },
                    "sourceFile": { "type": ["string", "null"] }
                }, "required": ["projectPath", "key", "files"], "additionalProperties": false
            })
        ),
        tool(
            "plan_detach",
            "Plan detaching one occurrence while preserving its current value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "linkId": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "linkId", "file"], "additionalProperties": false
            })
        ),
        tool(
            "plan_classification",
            "Plan an explicitly requested Codex access classification without a second confirmation round trip.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" },
                    "access": { "type": "string", "enum": ["read-write", "protected", "unclassified"] }
                }, "required": ["projectPath", "key", "access"], "additionalProperties": false
            })
        ),
        tool(
            "plan_migration",
            "Plan conversion of strong visual group comments to # @group without values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "file"], "additionalProperties": false
            })
        ),
        tool(
            "plan_copy_variable_from_project",
            "Plan a one-time opaque copy of one same-name value from another registered project. The value is handled only inside Rust and is never returned to the agent.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" },
                    "sourceProjectId": { "type": "string" },
                    "sourceFile": { "type": "string" },
                    "targetFile": { "type": "string" },
                    "key": { "type": "string" }
                },
                "required": ["projectPath", "sourceProjectId", "sourceFile", "targetFile", "key"],
                "additionalProperties": false
            })
        ),
        tool(
            "list_deployment_providers",
            "List official and locally installed providers with availability and version metadata. Never returns values or commands.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "list_action_packs",
            "List locally installed Action Packs, their required secret bindings, target metadata, and CLI compatibility. Never returns values, commands, arguments, or response bodies.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "list_runtime_targets",
            "List registered fixed-verifier Runtime targets for a project. Returns target IDs, display names, source files, and transport labels only; never returns recipients, destinations, remote paths, values, or commands.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "list_team_channels",
            "List connected Folder Team Channels and encrypted-package metadata for a registered project. Never returns folder paths, values, passphrases, or decrypted content. Passphrase publish/import remains a desktop action.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "compare_deployment_values",
            "Compare selected managed values with a supported deployment target. Returns equality states only; never accepts or returns candidate values, hashes, or provider output.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": { "type": "string" },
                    "provider": { "type": "string" },
                    "file": { "type": "string" },
                    "keys": {
                        "type": "array", "minItems": 1, "maxItems": 100,
                        "items": { "type": "string" }
                    },
                    "awsProfile": { "type": ["string", "null"] },
                    "awsRegion": { "type": ["string", "null"] },
                    "awsPathPrefix": { "type": ["string", "null"] }
                    ,"runtimeTargetId": { "type": ["string", "null"] }
                },
                "required": ["projectPath", "provider", "file", "keys"],
                "additionalProperties": false
            })
        ),
        tool(
            "plan_provider_push",
            "Create a redacted one-way provider push plan. Values remain inside Rust and are resolved only when apply_plan is called.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": { "type": "string" },
                    "provider": { "type": "string" },
                    "file": { "type": "string" },
                    "selections": {
                        "type": "array", "minItems": 1, "maxItems": 100,
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "kind": { "type": "string", "enum": ["secret", "variable", "plaintext", "sensitive"] }
                            },
                            "required": ["key", "kind"],
                            "additionalProperties": false
                        }
                    },
                    "repository": { "type": ["string", "null"] },
                    "githubEnvironment": { "type": ["string", "null"] },
                    "worker": { "type": ["string", "null"] },
                    "cloudflareEnvironment": { "type": ["string", "null"] },
                    "easProject": { "type": ["string", "null"] },
                    "easEnvironments": {
                        "type": "array", "maxItems": 10,
                        "items": { "type": "string" }
                    },
                    "personalTarget": { "type": ["string", "null"] }
                    ,"awsProfile": { "type": ["string", "null"] }
                    ,"awsRegion": { "type": ["string", "null"] }
                    ,"awsPathPrefix": { "type": ["string", "null"] }
                    ,"awsKmsKeyId": { "type": ["string", "null"] }
                },
                "required": ["projectPath", "provider", "file", "selections"],
                "additionalProperties": false
            })
        ),
        tool(
            "plan_action",
            "Create a redacted plan for one locally installed Action Pack. Bindings map pack binding IDs to managed variable names; raw values, commands, output, and response bodies are never accepted or returned.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": { "type": "string" },
                    "packId": { "type": "string" },
                    "file": { "type": "string" },
                    "bindings": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "minProperties": 1,
                        "maxProperties": 16
                    }
                },
                "required": ["projectPath", "packId", "file", "bindings"],
                "additionalProperties": false
            })
        ),
        tool(
            "apply_plan",
            "Apply one unexpired redacted plan authorized by the current user request.",
            json!({
                "type": "object", "properties": {
                    "planId": { "type": "string" }
                }, "required": ["planId"], "additionalProperties": false
            })
        )
    ])
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

use super::super::*;

pub fn guard_hook_decision(input: &Value) -> Value {
    if hook_requests_direct_env_access(input) {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "Direct env-file access is blocked by Kavranta. Use the env-manager MCP tools instead."
            }
        });
    }
    json!({})
}

fn hook_requests_direct_env_access(input: &Value) -> bool {
    let tool_name = input
        .get("tool_name")
        .or_else(|| input.get("toolName"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tool_input = input
        .get("tool_input")
        .or_else(|| input.get("toolInput"))
        .unwrap_or(&Value::Null);

    if contains_env_path_field(tool_input) {
        return true;
    }

    let command_like = tool_name.contains("bash")
        || tool_name.contains("shell")
        || tool_name.contains("terminal")
        || tool_name.contains("command")
        || tool_name.contains("apply_patch")
        || tool_name == "applypatch";
    command_like && contains_env_command_field(tool_input)
}

fn contains_env_path_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
            let path_field = matches!(
                normalized.as_str(),
                "path"
                    | "paths"
                    | "filepath"
                    | "filepaths"
                    | "uri"
                    | "uris"
                    | "glob"
                    | "globpattern"
                    | "include"
                    | "includes"
                    | "exclude"
                    | "excludes"
            );
            (path_field && value_contains_env_reference(value)) || contains_env_path_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_env_path_field),
        _ => false,
    }
}

fn contains_env_command_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
            let command_field = matches!(
                normalized.as_str(),
                "command" | "cmd" | "script" | "patch" | "patchtext"
            );
            (command_field && value_contains_env_reference(value))
                || contains_env_command_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_env_command_field),
        _ => false,
    }
}

fn value_contains_env_reference(value: &Value) -> bool {
    match value {
        Value::String(text) => contains_env_reference(text),
        Value::Array(values) => values.iter().any(value_contains_env_reference),
        Value::Object(fields) => fields.values().any(value_contains_env_reference),
        _ => false,
    }
}

fn contains_env_reference(text: &str) -> bool {
    contains_bounded_env_reference(text, ".env")
        || contains_bounded_env_reference(text, ".dev.vars")
}

fn contains_bounded_env_reference(text: &str, marker: &str) -> bool {
    text.match_indices(marker).any(|(index, _)| {
        let previous = text[..index].chars().next_back();
        let next = text[index + marker.len()..].chars().next();
        is_env_boundary_before(previous) && is_env_boundary_after(next)
    })
}

fn is_env_boundary_before(character: Option<char>) -> bool {
    character.is_none_or(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '/' | '\\' | '\'' | '"' | '`' | '=' | ':' | '(' | '[' | '{'
            )
    })
}

fn is_env_boundary_after(character: Option<char>) -> bool {
    character.is_none_or(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | '/' | '\\' | '\'' | '"' | '`' | ':' | ')' | ']' | '}' | ','
            )
    })
}

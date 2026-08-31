use std::io::{self, BufRead, Read, Write};

use env_broker::{
    Broker, apply_stdin_value_from_default_paths, guard_hook_decision, tool_definitions,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("guard-hook") => {
            run_guard_hook();
            return;
        }
        Some("--version" | "-V") => {
            println!("env-manager-broker {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("value") => {
            let exit_code = run_value_cli(std::env::args().skip(1).collect());
            std::process::exit(exit_code);
        }
        _ => {}
    }

    let broker = Broker::default();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Some(response) = handle_line(&broker, &line) else {
            continue;
        };
        if serde_json::to_writer(&mut stdout, &response).is_err()
            || writeln!(&mut stdout).is_err()
            || stdout.flush().is_err()
        {
            break;
        }
    }
}

fn run_value_cli(arguments: Vec<String>) -> i32 {
    let parsed = parse_apply_stdin_args(&arguments);
    let result = match parsed {
        Ok((plan_id, trim_final_newline)) => {
            apply_stdin_value_from_default_paths(&plan_id, trim_final_newline, io::stdin().lock())
                .and_then(|projection| {
                    serde_json::to_value(projection).map_err(|_| {
                        env_broker::StdinValueError::sanitized(
                            "SERIALIZATION_ERROR",
                            "저장 결과를 직렬화하지 못했습니다.",
                        )
                    })
                })
        }
        Err((code, message)) => {
            write_value_cli_output(json!({
                "succeeded": false,
                "resultCode": code,
                "message": message
            }));
            return 2;
        }
    };
    match result {
        Ok(projection) => {
            write_value_cli_output(json!({
                "succeeded": true,
                "result": projection
            }));
            0
        }
        Err(error) => {
            write_value_cli_output(json!({
                "succeeded": false,
                "resultCode": error.code(),
                "message": error.message()
            }));
            2
        }
    }
}

fn parse_apply_stdin_args(
    arguments: &[String],
) -> Result<(String, bool), (&'static str, &'static str)> {
    if arguments.first().map(String::as_str) != Some("value")
        || arguments.get(1).map(String::as_str) != Some("apply-stdin")
    {
        return Err(("INVALID_REQUEST", "지원하지 않는 value 명령입니다."));
    }
    let mut plan_id = None;
    let mut trim_final_newline = false;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--plan" if plan_id.is_none() => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err(("INVALID_REQUEST", "--plan 값이 필요합니다."));
                };
                plan_id = Some(value.clone());
                index += 2;
            }
            "--trim-final-newline" if !trim_final_newline => {
                trim_final_newline = true;
                index += 1;
            }
            _ => {
                return Err((
                    "INVALID_REQUEST",
                    "value apply-stdin 인자가 올바르지 않습니다.",
                ));
            }
        }
    }
    plan_id
        .map(|plan_id| (plan_id, trim_final_newline))
        .ok_or(("INVALID_REQUEST", "--plan 값이 필요합니다."))
}

fn write_value_cli_output(value: Value) {
    let mut stdout = io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &value).is_ok() {
        let _ = writeln!(&mut stdout);
        let _ = stdout.flush();
    }
}

fn run_guard_hook() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return;
    }
    let event = serde_json::from_str::<Value>(&input).unwrap_or(Value::Null);
    let decision = guard_hook_decision(&event);
    let mut stdout = io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &decision).is_ok() {
        let _ = writeln!(&mut stdout);
        let _ = stdout.flush();
    }
}

fn handle_line(broker: &Broker, line: &str) -> Option<Value> {
    let request = match serde_json::from_str::<RpcRequest>(line) {
        Ok(request) => request,
        Err(_) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "Invalid JSON-RPC request" }
            }));
        }
    };

    let id = request.id?;
    let result = match request.method.as_str() {
        "initialize" => {
            if let Some(client_name) = request
                .params
                .pointer("/clientInfo/name")
                .and_then(Value::as_str)
            {
                broker.identify_client(client_name);
            }
            json!({
                "protocolVersion": request.params.get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("2025-06-18"),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "env-manager", "version": env!("CARGO_PKG_VERSION") }
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_definitions() }),
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match broker.call_tool(name, arguments) {
                Ok(data) => json!({
                    "content": [{ "type": "text", "text": data.to_string() }],
                    "structuredContent": data,
                    "isError": false
                }),
                Err(error) => json!({
                    "content": [{
                        "type": "text",
                        "text": json!({
                            "code": error.code().as_str(),
                            "message": error.to_string()
                        }).to_string()
                    }],
                    "isError": true
                }),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_stdin_cli_accepts_only_the_closed_argument_shape() {
        let plan = "stdin-plan-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            parse_apply_stdin_args(&[
                "value".to_owned(),
                "apply-stdin".to_owned(),
                "--plan".to_owned(),
                plan.to_owned(),
                "--trim-final-newline".to_owned(),
            ]),
            Ok((plan.to_owned(), true))
        );
        for invalid in [
            vec!["value".to_owned(), "apply-stdin".to_owned()],
            vec![
                "value".to_owned(),
                "apply-stdin".to_owned(),
                "--command".to_owned(),
                "cat".to_owned(),
            ],
            vec![
                "value".to_owned(),
                "apply-stdin".to_owned(),
                "--plan".to_owned(),
                plan.to_owned(),
                "--plan".to_owned(),
                plan.to_owned(),
            ],
        ] {
            assert!(parse_apply_stdin_args(&invalid).is_err());
        }
    }
}

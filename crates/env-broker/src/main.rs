use std::io::{self, BufRead, Write};

use env_broker::{Broker, tool_definitions};
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
        "initialize" => json!({
            "protocolVersion": request.params.get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-06-18"),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "env-manager", "version": env!("CARGO_PKG_VERSION") }
        }),
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

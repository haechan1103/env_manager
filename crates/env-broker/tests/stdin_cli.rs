use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use env_core::ProjectService;
use env_registry::{ProjectRegistration, RegistryData};
use env_test_support::SyntheticProject;
use serde_json::{Value, json};

#[test]
fn broker_plan_and_separate_cli_process_complete_an_opaque_stdin_write() {
    let project = SyntheticProject::new();
    project.write(".env.local", "AUTH_SECRET=fake_before\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    let app_data = tempfile::tempdir().expect("app data");
    let registry_path = app_data.path().join("projects.json");
    env_registry::write(
        &registry_path,
        &RegistryData {
            projects: vec![ProjectRegistration {
                id: service.project_id().to_owned(),
                name: "Synthetic".to_owned(),
                display_path: service.root().to_string_lossy().into_owned(),
                root: service.root().to_path_buf(),
                file_labels: Default::default(),
            }],
            ..RegistryData::default()
        },
    )
    .expect("registry");

    let binary = env!("CARGO_BIN_EXE_env-manager-broker");
    let mut mcp = Command::new(binary)
        .current_dir(project.root())
        .env("ENV_MANAGER_APP_DATA_DIR", app_data.path())
        .env("ENV_MANAGER_REGISTRY_PATH", &registry_path)
        .env("ENV_MANAGER_AGENT_HOST", "codex")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn MCP broker");
    let mut mcp_stdin = mcp.stdin.take().expect("MCP stdin");
    let mut mcp_stdout = BufReader::new(mcp.stdout.take().expect("MCP stdout"));
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "plan_stdin_value_write",
            "arguments": {
                "projectPath": project.root(),
                "file": ".env.local",
                "key": "AUTH_SECRET",
                "trimFinalNewline": true
            }
        }
    });
    serde_json::to_writer(&mut mcp_stdin, &request).expect("write request");
    writeln!(&mut mcp_stdin).expect("request newline");
    mcp_stdin.flush().expect("flush request");
    let mut response_line = String::new();
    mcp_stdout
        .read_line(&mut response_line)
        .expect("read response");
    let response: Value = serde_json::from_str(&response_line).expect("response JSON");
    let projection = &response["result"]["structuredContent"];
    let plan_id = projection["planId"].as_str().expect("plan id");
    let broker_executable = projection["brokerExecutable"]
        .as_str()
        .expect("broker executable");
    assert_eq!(broker_executable, binary);
    assert!(!response_line.contains("fake_before"));

    let mut apply = Command::new(broker_executable)
        .args([
            "value",
            "apply-stdin",
            "--plan",
            plan_id,
            "--trim-final-newline",
        ])
        .env("ENV_MANAGER_APP_DATA_DIR", app_data.path())
        .env("ENV_MANAGER_REGISTRY_PATH", &registry_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn stdin apply");
    apply
        .stdin
        .take()
        .expect("apply stdin")
        .write_all(b"fake_cli_canary_931f\n")
        .expect("write fake producer value");
    let output = apply.wait_with_output().expect("wait for apply");
    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(output_text.contains(r#""resultCode":"OK""#));
    assert!(!output_text.contains("fake_cli_canary_931f"));
    assert_eq!(
        project.read(".env.local"),
        b"AUTH_SECRET=fake_cli_canary_931f\n"
    );

    drop(mcp_stdin);
    let _ = mcp.kill();
    let _ = mcp.wait();
}

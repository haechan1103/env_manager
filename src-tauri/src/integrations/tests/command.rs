use super::super::command::executable_file_names;

#[test]
fn windows_agent_cli_candidates_include_native_and_script_launchers() {
    assert_eq!(
        executable_file_names("codex", true),
        vec!["codex.exe", "codex.cmd", "codex.bat"]
    );
    assert_eq!(executable_file_names("codex", false), vec!["codex"]);
}

#[cfg(windows)]
#[test]
fn windows_cmd_agent_launcher_executes_with_literal_arguments() {
    use std::ffi::OsString;
    use std::fs;

    use super::super::command::agent_command;

    let directory = tempfile::tempdir().expect("temporary directory");
    let launcher = directory.path().join("fake-agent.cmd");
    fs::write(
        &launcher,
        "@echo off\r\nif \"%~1\"==\"--version\" exit /b 0\r\nexit /b 1\r\n",
    )
    .expect("write launcher");

    let status = agent_command(&launcher, &[OsString::from("--version")])
        .status()
        .expect("run launcher");

    assert!(status.success());
}

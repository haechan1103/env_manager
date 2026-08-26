use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use env_core::ProviderValue;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use crate::provider_adapter::{self, AdapterStrategy, ResolvedAdapter};

use super::cli::provider_command;
use super::error::{ProviderPushError, invalid_request, invalid_target};
use super::model::{
    EXPO_EAS_ID, EasAccessContext, EasTargetContext, ProviderEntryKind, ProviderPushRequest,
    ProviderPushResult,
};
use super::validation::{source_directory, validate_simple_target};

const PROMPT: &[u8] = b"Variable value:";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_PREFLIGHT_OUTPUT: usize = 64 * 1024;
const MAX_PROMPT_WINDOW: usize = 4096;

pub(super) struct PreparedEasProvider {
    root: PathBuf,
    adapter: ResolvedAdapter,
}

pub fn detect_target(
    root: &Path,
    source_file: &str,
) -> Result<EasTargetContext, ProviderPushError> {
    let source = source_directory(root, source_file)?;
    let eas_json = find_upward(&source, root, "eas.json").ok_or(ProviderPushError {
        code: "EAS_CONFIG_NOT_FOUND",
        message: "선택한 환경 파일과 연결된 eas.json을 찾지 못했습니다.",
    })?;
    let eas_root = eas_json.parent().ok_or_else(invalid_target)?;
    let environments = parse_environments(&eas_json)?;
    let (project, project_id) = detect_project_metadata(eas_root);
    let config_path = eas_json
        .strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    Ok(EasTargetContext {
        project,
        project_id,
        environments,
        config_path,
    })
}

pub fn inspect_access(
    root: &Path,
    app_data: &Path,
    source_file: &str,
    expected_project: Option<&str>,
) -> Result<EasAccessContext, ProviderPushError> {
    let (target, eas_root, adapter) = resolve_adapter(root, app_data, source_file)?;
    let actual = inspect_project(&adapter, &eas_root)?;
    let expected = expected_project
        .filter(|value| !value.trim().is_empty())
        .or(target.project.as_deref())
        .or(target.project_id.as_deref())
        .ok_or(ProviderPushError {
            code: "EAS_PROJECT_REQUIRED",
            message: "전송할 Expo EAS 프로젝트를 확인해주세요.",
        })?;
    if !configured_project_matches(&target, &actual) || !project_matches(expected, &actual) {
        return Err(ProviderPushError {
            code: "EAS_PROJECT_MISMATCH",
            message: "로그인된 Expo EAS 프로젝트가 선택한 대상과 일치하지 않습니다.",
        });
    }
    Ok(EasAccessContext {
        project: actual.full_name,
        project_id: actual.id,
        adapter: crate::provider_adapter::AdapterStatus::from(&adapter),
    })
}

pub(super) fn prepare(
    root: &Path,
    app_data: &Path,
    request: &ProviderPushRequest,
) -> Result<PreparedEasProvider, ProviderPushError> {
    validate_request(request)?;
    let (target, eas_root, adapter) = resolve_adapter(root, app_data, &request.file)?;
    let actual = inspect_project(&adapter, &eas_root)?;
    let expected = request.eas_project.as_deref().ok_or(ProviderPushError {
        code: "EAS_PROJECT_REQUIRED",
        message: "전송할 Expo EAS 프로젝트를 확인해주세요.",
    })?;
    if !configured_project_matches(&target, &actual) || !project_matches(expected, &actual) {
        return Err(ProviderPushError {
            code: "EAS_PROJECT_MISMATCH",
            message: "로그인된 Expo EAS 프로젝트가 선택한 대상과 일치하지 않습니다.",
        });
    }
    Ok(PreparedEasProvider {
        root: eas_root,
        adapter,
    })
}

fn resolve_adapter(
    root: &Path,
    app_data: &Path,
    source_file: &str,
) -> Result<(EasTargetContext, PathBuf, ResolvedAdapter), ProviderPushError> {
    let target = detect_target(root, source_file)?;
    let config = target.config_path.as_deref().ok_or_else(invalid_target)?;
    let eas_root = root
        .join(config)
        .parent()
        .ok_or_else(invalid_target)?
        .to_owned();
    let adapter = provider_adapter::resolve(
        crate::provider_push::OfficialProviderId::ExpoEas,
        &eas_root,
        app_data,
    )?;
    if adapter.strategy != AdapterStrategy::EasEnvSetPromptV1 {
        return Err(ProviderPushError {
            code: "PROVIDER_ADAPTER_INVALID",
            message: "Expo EAS Adapter 전략이 올바르지 않습니다.",
        });
    }
    Ok((target, eas_root, adapter))
}

pub(super) fn push(
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
    prepared: PreparedEasProvider,
) -> Result<ProviderPushResult, ProviderPushError> {
    let kinds = request
        .selections
        .iter()
        .map(|selection| (selection.key.as_str(), selection.kind))
        .collect::<BTreeMap<_, _>>();
    let mut pushed_count = 0;
    let mut failed_keys = Vec::new();
    for value in &values {
        let kind = kinds.get(value.key()).copied().ok_or_else(invalid_target)?;
        let args = set_args(value.key(), kind, &request.eas_environments)?;
        if execute_hidden_prompt(
            &prepared.adapter.executable,
            &prepared.root,
            &args,
            value.value(),
        ) {
            pushed_count += 1;
        } else {
            failed_keys.push(value.key().to_owned());
        }
    }
    Ok(ProviderPushResult {
        provider: EXPO_EAS_ID.to_owned(),
        pushed_count,
        failed_keys,
    })
}

fn validate_request(request: &ProviderPushRequest) -> Result<(), ProviderPushError> {
    let environments = request.eas_environments.iter().collect::<BTreeSet<_>>();
    if request.eas_environments.is_empty()
        || request.eas_environments.len() > 10
        || environments.len() != request.eas_environments.len()
    {
        return Err(invalid_request(
            "EAS 환경을 1개 이상 중복 없이 선택해주세요.",
        ));
    }
    for environment in &request.eas_environments {
        validate_simple_target(environment)?;
    }
    for selection in &request.selections {
        validate_simple_target(&selection.key)?;
        if selection.kind == ProviderEntryKind::Variable {
            return Err(invalid_request(
                "Expo EAS는 Plain text, Sensitive 또는 Secret만 지원합니다.",
            ));
        }
        if selection.key.starts_with("EXPO_PUBLIC_") && selection.kind == ProviderEntryKind::Secret
        {
            return Err(ProviderPushError {
                code: "EAS_PUBLIC_SECRET_UNSUPPORTED",
                message: "EXPO_PUBLIC_ 변수는 앱 번들에 필요한 공개 식별자이므로 EAS Secret으로 전송할 수 없습니다.",
            });
        }
    }
    Ok(())
}

fn set_args(
    key: &str,
    kind: ProviderEntryKind,
    environments: &[String],
) -> Result<Vec<OsString>, ProviderPushError> {
    validate_simple_target(key)?;
    let visibility = match kind {
        ProviderEntryKind::Plaintext => "plaintext",
        ProviderEntryKind::Sensitive => "sensitive",
        ProviderEntryKind::Secret if !key.starts_with("EXPO_PUBLIC_") => "secret",
        ProviderEntryKind::Secret => {
            return Err(ProviderPushError {
                code: "EAS_PUBLIC_SECRET_UNSUPPORTED",
                message: "EXPO_PUBLIC_ 변수는 앱 번들에 필요한 공개 식별자이므로 EAS Secret으로 전송할 수 없습니다.",
            });
        }
        ProviderEntryKind::Variable => {
            return Err(invalid_request(
                "Expo EAS에서 지원하지 않는 변수 유형입니다.",
            ));
        }
    };
    let mut args = vec![
        OsString::from("env:set"),
        OsString::from("--name"),
        OsString::from(key),
        OsString::from("--type"),
        OsString::from("string"),
        OsString::from("--visibility"),
        OsString::from(visibility),
        OsString::from("--scope"),
        OsString::from("project"),
    ];
    for environment in environments {
        args.push(OsString::from("--environment"));
        args.push(OsString::from(environment));
    }
    Ok(args)
}

fn execute_hidden_prompt(executable: &Path, root: &Path, args: &[OsString], value: &str) -> bool {
    let system = native_pty_system();
    let pair = match system.openpty(PtySize {
        rows: 24,
        cols: 100,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(_) => return false,
    };
    let mut command = pty_command(executable, args);
    command.cwd(root);
    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(_) => return false,
    };
    drop(pair.slave);
    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => {
            let _ = child.kill();
            return false;
        }
    };
    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => {
            let _ = child.kill();
            return false;
        }
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader_thread = thread::spawn(move || {
        let mut chunk = Zeroizing::new([0_u8; 512]);
        let mut window = Zeroizing::new(Vec::<u8>::with_capacity(MAX_PROMPT_WINDOW));
        let mut reported = false;
        while let Ok(count) = reader.read(&mut *chunk) {
            if count == 0 {
                break;
            }
            if !reported {
                window.extend_from_slice(&chunk[..count]);
                if window
                    .windows(PROMPT.len())
                    .any(|candidate| candidate == PROMPT)
                {
                    let _ = sender.send(());
                    reported = true;
                    window.zeroize();
                } else if window.len() > MAX_PROMPT_WINDOW {
                    let keep = PROMPT.len().saturating_sub(1);
                    let discard = window.len().saturating_sub(keep);
                    window.drain(..discard);
                }
            }
            chunk[..count].zeroize();
        }
    });

    let prompt_deadline = Instant::now() + PROMPT_TIMEOUT;
    let prompted = loop {
        if receiver.recv_timeout(Duration::from_millis(50)).is_ok() {
            break true;
        }
        if child.try_wait().ok().flatten().is_some() || Instant::now() >= prompt_deadline {
            break false;
        }
    };
    if !prompted
        || writer.write_all(value.as_bytes()).is_err()
        || writer.write_all(b"\r").is_err()
        || writer.flush().is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        drop(writer);
        let _ = reader_thread.join();
        return false;
    }
    let completion_deadline = Instant::now() + COMPLETION_TIMEOUT;
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < completion_deadline => {
                thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    drop(writer);
    let _ = reader_thread.join();
    success
}

fn pty_command(executable: &Path, args: &[OsString]) -> CommandBuilder {
    if cfg!(windows)
        && executable.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        let mut command = CommandBuilder::new("cmd.exe");
        command.args([
            OsString::from("/D"),
            OsString::from("/S"),
            OsString::from("/C"),
        ]);
        command.arg(executable.as_os_str());
        command.args(args.iter());
        command
    } else {
        let mut command = CommandBuilder::new(executable.as_os_str());
        command.args(args.iter());
        command
    }
}

fn inspect_project(
    adapter: &ResolvedAdapter,
    root: &Path,
) -> Result<EasProjectInfo, ProviderPushError> {
    let args = [
        OsString::from("project:info"),
        OsString::from("--non-interactive"),
    ];
    let output = provider_command(&adapter.executable, &args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| eas_access_error())?;
    if !output.status.success() || output.stdout.len() > MAX_PREFLIGHT_OUTPUT {
        return Err(eas_access_error());
    }
    parse_project_info(&output.stdout).ok_or_else(eas_access_error)
}

#[derive(Debug, PartialEq, Eq)]
struct EasProjectInfo {
    full_name: String,
    id: String,
}

fn parse_project_info(output: &[u8]) -> Option<EasProjectInfo> {
    let text = std::str::from_utf8(output).ok()?;
    let mut full_name = None;
    let mut id = None;
    for line in text.lines() {
        let clean = strip_ansi(line);
        let trimmed = clean.trim();
        if let Some(value) = trimmed.strip_prefix("fullName") {
            full_name = value
                .trim_matches(|c: char| c.is_whitespace() || c == ':' || c == '=')
                .split_whitespace()
                .next()
                .map(str::to_owned);
        } else if let Some(value) = trimmed.strip_prefix("ID") {
            id = value
                .trim_matches(|c: char| c.is_whitespace() || c == ':' || c == '=')
                .split_whitespace()
                .next()
                .map(str::to_owned);
        }
    }
    Some(EasProjectInfo {
        full_name: full_name?,
        id: id?,
    })
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.next() == Some('[') {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(character);
        }
    }
    output
}

fn project_matches(expected: &str, actual: &EasProjectInfo) -> bool {
    let expected = expected.trim();
    expected == actual.id
        || expected == actual.full_name
        || expected.trim_start_matches('@').split('/').next_back()
            == actual
                .full_name
                .trim_start_matches('@')
                .split('/')
                .next_back()
}

fn configured_project_matches(target: &EasTargetContext, actual: &EasProjectInfo) -> bool {
    target
        .project_id
        .as_deref()
        .is_none_or(|project_id| project_id == actual.id)
}

fn parse_environments(path: &Path) -> Result<Vec<String>, ProviderPushError> {
    let input = std::fs::read(path).map_err(|_| invalid_target())?;
    let value: Value = serde_json::from_slice(&input).map_err(|_| ProviderPushError {
        code: "EAS_CONFIG_INVALID",
        message: "eas.json 형식을 읽지 못했습니다.",
    })?;
    let mut environments = BTreeSet::new();
    if let Some(build) = value.get("build").and_then(Value::as_object) {
        for profile in build.values() {
            if let Some(environment) = profile.get("environment").and_then(Value::as_str)
                && validate_simple_target(environment).is_ok()
            {
                environments.insert(environment.to_owned());
            }
        }
    }
    if environments.is_empty() {
        environments.extend(["development", "preview", "production"].map(str::to_owned));
    }
    Ok(environments.into_iter().collect())
}

fn detect_project_metadata(root: &Path) -> (Option<String>, Option<String>) {
    for name in ["app.json", "app.config.json"] {
        let path = root.join(name);
        if let Ok(input) = std::fs::read(&path)
            && let Ok(value) = serde_json::from_slice::<Value>(&input)
        {
            let expo = value.get("expo").unwrap_or(&value);
            let project = expo.get("slug").and_then(Value::as_str).map(str::to_owned);
            let project_id = expo
                .pointer("/extra/eas/projectId")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if project.is_some() || project_id.is_some() {
                return (project, project_id);
            }
        }
    }
    for name in ["app.config.ts", "app.config.js"] {
        if let Ok(text) = std::fs::read_to_string(root.join(name)) {
            let project = extract_js_string(&text, "slug");
            let project_id = extract_js_string(&text, "projectId");
            if project.is_some() || project_id.is_some() {
                return (project, project_id);
            }
        }
    }
    (None, None)
}

fn extract_js_string(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)? + key.len();
    let tail = &text[start..];
    let separator = tail.find(':')?;
    let tail = tail[separator + 1..].trim_start();
    let quote = tail.chars().next()?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    let rest = &tail[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_owned())
}

fn find_upward(start: &Path, root: &Path, name: &str) -> Option<PathBuf> {
    let mut current = start.to_owned();
    loop {
        let candidate = current.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if current == root || !current.pop() || !current.starts_with(root) {
            return None;
        }
    }
}

fn eas_access_error() -> ProviderPushError {
    ProviderPushError {
        code: "EAS_ACCESS_UNAVAILABLE",
        message: "Expo 로그인이 없거나 현재 계정에 EAS 프로젝트 권한이 없습니다.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_eas_target_without_reading_env_values() {
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(project.path().join("apps/mobile")).expect("mobile");
        std::fs::write(
            project.path().join("apps/mobile/.env"),
            "SYNTHETIC_KEY=canary\n",
        )
        .expect("env");
        std::fs::write(project.path().join("apps/mobile/eas.json"), r#"{"build":{"dev":{"environment":"development"},"prod":{"environment":"production"}}}"#).expect("eas");
        std::fs::write(
            project.path().join("apps/mobile/app.json"),
            r#"{"expo":{"slug":"travel-pieces","extra":{"eas":{"projectId":"synthetic-id"}}}}"#,
        )
        .expect("app");

        let target = detect_target(project.path(), "apps/mobile/.env").expect("target");
        assert_eq!(target.project.as_deref(), Some("travel-pieces"));
        assert_eq!(target.project_id.as_deref(), Some("synthetic-id"));
        assert_eq!(target.environments, vec!["development", "production"]);
    }

    #[test]
    fn eas_arguments_never_contain_the_value_or_value_flag() {
        let canary = "synthetic-eas-secret-canary";
        let args = set_args(
            "EXPO_PUBLIC_KAKAO_NATIVE_APP_KEY",
            ProviderEntryKind::Sensitive,
            &[
                "development".to_owned(),
                "preview".to_owned(),
                "production".to_owned(),
            ],
        )
        .expect("args");
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!rendered.contains(canary));
        assert!(!rendered.contains("--value"));
        assert!(rendered.contains("--visibility sensitive"));
    }

    #[test]
    fn expo_public_cannot_be_misclassified_as_eas_secret() {
        let error = set_args(
            "EXPO_PUBLIC_KAKAO_NATIVE_APP_KEY",
            ProviderEntryKind::Secret,
            &["production".to_owned()],
        )
        .expect_err("reject secret");
        assert_eq!(error.code, "EAS_PUBLIC_SECRET_UNSUPPORTED");
    }

    #[test]
    fn parses_and_matches_project_info() {
        let info = parse_project_info(
            b"fullName  @haechan/travel-pieces\nID  2bb051f4-155a-4978-a1b5-934596bd8f3a\n",
        )
        .expect("info");
        assert!(project_matches("travel-pieces", &info));
        assert!(project_matches("@haechan/travel-pieces", &info));
        assert!(!project_matches("other-project", &info));
        assert!(configured_project_matches(
            &EasTargetContext {
                project: Some("travel-pieces".to_owned()),
                project_id: Some("2bb051f4-155a-4978-a1b5-934596bd8f3a".to_owned()),
                environments: Vec::new(),
                config_path: Some("apps/mobile/eas.json".to_owned()),
            },
            &info,
        ));
        assert!(!configured_project_matches(
            &EasTargetContext {
                project: Some("travel-pieces".to_owned()),
                project_id: Some("different-project-id".to_owned()),
                environments: Vec::new(),
                config_path: Some("apps/mobile/eas.json".to_owned()),
            },
            &info,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn pty_transport_waits_for_prompt_and_keeps_canary_out_of_arguments() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("directory");
        let executable = directory.path().join("fake-eas");
        std::fs::write(&executable, "#!/bin/sh\nprintf 'Variable value:'\nIFS= read -r value\n[ \"$value\" = \"synthetic-eas-pty-canary\" ]\n").expect("script");
        let mut permissions = std::fs::metadata(&executable)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&executable, permissions).expect("permissions");
        let args = set_args(
            "SYNTHETIC_KEY",
            ProviderEntryKind::Sensitive,
            &["development".to_owned()],
        )
        .expect("args");

        assert!(execute_hidden_prompt(
            &executable,
            directory.path(),
            &args,
            "synthetic-eas-pty-canary"
        ));
        assert!(
            args.iter()
                .all(|arg| !arg.to_string_lossy().contains("synthetic-eas-pty-canary"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_conpty_transport_waits_for_hidden_prompt() {
        let directory = tempfile::tempdir().expect("directory");
        let executable = directory.path().join("fake-eas.cmd");
        std::fs::write(
            &executable,
            "@echo off\r\n<nul set /p \"=Variable value:\"\r\nset /p \"value=\"\r\nif \"%value%\"==\"synthetic-eas-pty-canary\" exit /b 0\r\nexit /b 1\r\n",
        )
        .expect("script");
        let args = set_args(
            "SYNTHETIC_KEY",
            ProviderEntryKind::Sensitive,
            &["development".to_owned()],
        )
        .expect("args");

        assert!(execute_hidden_prompt(
            &executable,
            directory.path(),
            &args,
            "synthetic-eas-pty-canary"
        ));
        assert!(
            args.iter()
                .all(|arg| !arg.to_string_lossy().contains("synthetic-eas-pty-canary"))
        );
    }
}

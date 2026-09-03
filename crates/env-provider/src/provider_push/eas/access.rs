use super::*;

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

pub(super) fn resolve_adapter(
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

pub(super) fn inspect_project(
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
pub(super) struct EasProjectInfo {
    full_name: String,
    id: String,
}

pub(super) fn parse_project_info(output: &[u8]) -> Option<EasProjectInfo> {
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

pub(super) fn strip_ansi(input: &str) -> String {
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

pub(super) fn project_matches(expected: &str, actual: &EasProjectInfo) -> bool {
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

pub(super) fn configured_project_matches(
    target: &EasTargetContext,
    actual: &EasProjectInfo,
) -> bool {
    target
        .project_id
        .as_deref()
        .is_none_or(|project_id| project_id == actual.id)
}

pub(super) fn eas_access_error() -> ProviderPushError {
    ProviderPushError {
        code: "EAS_ACCESS_UNAVAILABLE",
        message: "Expo 로그인이 없거나 현재 계정에 EAS 프로젝트 권한이 없습니다.",
    }
}

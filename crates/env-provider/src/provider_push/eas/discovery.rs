use super::*;

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

pub(super) fn parse_environments(path: &Path) -> Result<Vec<String>, ProviderPushError> {
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

pub(super) fn detect_project_metadata(root: &Path) -> (Option<String>, Option<String>) {
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

pub(super) fn extract_js_string(text: &str, key: &str) -> Option<String> {
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

pub(super) fn find_upward(start: &Path, root: &Path, name: &str) -> Option<PathBuf> {
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

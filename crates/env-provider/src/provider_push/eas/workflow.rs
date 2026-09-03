use super::*;

pub(in crate::provider_push) fn prepare(
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

pub(in crate::provider_push) fn push(
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

pub(super) fn validate_request(request: &ProviderPushRequest) -> Result<(), ProviderPushError> {
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

use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMutation<T> {
    pub(super) project_id: String,
    pub(super) request: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetachRequest {
    project_id: String,
    link_id: String,
    file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationRequest {
    project_id: String,
    key: String,
    access: CodexAccess,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProtectionRequest {
    project_id: String,
    keys: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueRequest {
    project_id: String,
    file: String,
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyRequest {
    project_id: String,
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreviewRequest {
    project_id: String,
    file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyMigrationRequest {
    project_id: String,
    plan_id: String,
    confirmed: bool,
}

#[tauri::command]
pub fn scan_project(
    request: ProjectRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<ProjectProjection> {
    let projection = runtime.scan(&request.project_id)?;
    let paths = projection
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    runtime.start_watching(&app, &request.project_id, &paths)?;
    Ok(projection)
}

#[tauri::command]
pub fn apply_gitignore_guard(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<GitignoreUpdateSummary> {
    runtime
        .service(&request.project_id)?
        .apply_gitignore_guard()
        .map_err(Into::into)
}

#[tauri::command]
pub fn save_value(
    payload: ProjectMutation<SaveValueRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .save_value(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn save_description(
    payload: ProjectMutation<SaveDescriptionRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .save_description(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn create_group(
    payload: ProjectMutation<CreateGroupRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .create_group(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn rename_group(
    payload: ProjectMutation<RenameGroupRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .rename_group(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn add_variable(
    payload: ProjectMutation<AddVariableRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .add_variable(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn delete_variable(
    payload: ProjectMutation<DeleteVariableRequest>,
    confirmed: bool,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    if !confirmed {
        return Err(EnvError::invalid("변수 삭제에는 확인이 필요합니다.").into());
    }
    runtime
        .service(&payload.project_id)?
        .delete_variable(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn move_variable(
    payload: ProjectMutation<MoveVariableRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .move_variable(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn create_link(
    payload: ProjectMutation<LinkRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    runtime
        .service(&payload.project_id)?
        .create_link(payload.request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn detach_link_member(
    request: DetachRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    runtime
        .service(&request.project_id)?
        .detach_link_member(&request.link_id, &request.file)
        .map_err(Into::into)
}

#[tauri::command]
pub fn set_codex_access(
    request: ClassificationRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    if request.access == CodexAccess::ReadWrite && !request.confirmed {
        return Err(EnvError::confirmation_required(&request.key).into());
    }
    runtime
        .service(&request.project_id)?
        .set_codex_access(&request.key, request.access)
        .map_err(Into::into)
}

#[tauri::command]
pub fn protect_variables(
    request: BatchProtectionRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    runtime
        .service(&request.project_id)?
        .set_codex_access_batch(&request.keys, CodexAccess::Protected)
        .map_err(Into::into)
}

#[tauri::command]
pub fn list_agent_activity(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<AgentActivityEvent>> {
    runtime
        .agent_activity(&request.project_id)
        .map_err(Into::into)
}

#[tauri::command]
pub fn read_value(request: ValueRequest, runtime: State<'_, AppRuntime>) -> CommandResult<String> {
    runtime
        .service(&request.project_id)?
        .read_value(&request.file, &request.key)
        .map_err(Into::into)
}

#[tauri::command]
pub fn copy_value(request: ValueRequest, runtime: State<'_, AppRuntime>) -> CommandResult<()> {
    let value = runtime
        .service(&request.project_id)?
        .read_value(&request.file, &request.key)?;
    let mut clipboard = arboard::Clipboard::new().map_err(|_| CommandError {
        code: "CLIPBOARD_UNAVAILABLE".to_owned(),
        message: "클립보드를 사용할 수 없습니다.".to_owned(),
    })?;
    clipboard
        .set_text(value.clone())
        .map_err(|_| CommandError {
            code: "CLIPBOARD_WRITE_FAILED".to_owned(),
            message: "클립보드에 복사하지 못했습니다.".to_owned(),
        })?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(45));
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && clipboard.get_text().is_ok_and(|current| current == value)
        {
            let _ = clipboard.set_text(String::new());
        }
    });
    Ok(())
}

#[tauri::command]
pub fn copy_key(request: KeyRequest, runtime: State<'_, AppRuntime>) -> CommandResult<()> {
    runtime
        .service(&request.project_id)?
        .codex_access(&request.key)?;
    let mut clipboard = arboard::Clipboard::new().map_err(|_| CommandError {
        code: "CLIPBOARD_UNAVAILABLE".to_owned(),
        message: "클립보드를 사용할 수 없습니다.".to_owned(),
    })?;
    clipboard.set_text(request.key).map_err(|_| CommandError {
        code: "CLIPBOARD_WRITE_FAILED".to_owned(),
        message: "환경변수명을 클립보드에 복사하지 못했습니다.".to_owned(),
    })
}

#[tauri::command]
pub fn plan_migration(
    request: MigrationPreviewRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MigrationPlanProjection> {
    runtime
        .plan_migration(&request.project_id, &request.file)
        .map_err(Into::into)
}

#[tauri::command]
pub fn apply_migration(
    request: ApplyMigrationRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<MutationSummary> {
    if !request.confirmed {
        return Err(EnvError::invalid("정리 계획 적용에는 확인이 필요합니다.").into());
    }
    runtime
        .apply_migration(&request.project_id, &request.plan_id)
        .map_err(Into::into)
}

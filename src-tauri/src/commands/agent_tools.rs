use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationRequest {
    id: AgentIntegrationId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPersonalProviderRequest {
    path: String,
    replace: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovePersonalProviderRequest {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteActionPackRequest {
    project_id: String,
    request: ActionExecutionRequest,
}

#[tauri::command]
pub fn list_agent_integrations(app: AppHandle) -> Vec<integrations::AgentIntegrationStatus> {
    integrations::list(&app)
}

#[tauri::command]
pub fn install_personal_provider_pack(
    request: InstallPersonalProviderRequest,
    app: AppHandle,
) -> CommandResult<env_provider::personal_provider::PersonalProviderPackInfo> {
    let app_data = provider_app_data(&app)?;
    env_provider::personal_provider::install(Path::new(&request.path), &app_data, request.replace)
        .map_err(Into::into)
}

#[tauri::command]
pub fn remove_personal_provider_pack(
    request: RemovePersonalProviderRequest,
    app: AppHandle,
) -> CommandResult<()> {
    let app_data = provider_app_data(&app)?;
    env_provider::personal_provider::remove(&request.id, &app_data).map_err(Into::into)
}

#[tauri::command]
pub fn install_action_pack(
    request: InstallPersonalProviderRequest,
    app: AppHandle,
) -> CommandResult<action_pack::ActionPackInfo> {
    let app_data = provider_app_data(&app)?;
    action_pack::install(Path::new(&request.path), &app_data, request.replace).map_err(Into::into)
}

#[tauri::command]
pub fn remove_action_pack(
    request: RemovePersonalProviderRequest,
    app: AppHandle,
) -> CommandResult<()> {
    let app_data = provider_app_data(&app)?;
    action_pack::remove(&request.id, &app_data).map_err(Into::into)
}

#[tauri::command]
pub async fn list_action_packs(
    request: ProjectRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<action_pack::ActionPackInfo>> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || action_pack::list(&root, &app_data))
        .await
        .map_err(|_| provider_task_interrupted())
}

#[tauri::command]
pub async fn execute_action_pack(
    payload: ExecuteActionPackRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<action_pack::ActionExecutionResult> {
    let service = runtime.service(&payload.project_id)?;
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        action_pack::execute(&service, &app_data, payload.request)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub fn install_agent_integration(
    request: AgentIntegrationRequest,
    app: AppHandle,
) -> CommandResult<integrations::AgentIntegrationStatus> {
    integrations::install(&app, request.id).map_err(Into::into)
}

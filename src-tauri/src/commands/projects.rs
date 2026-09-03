use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRequest {
    pub(super) project_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedProjectRequest {
    project_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameProjectRequest {
    project_id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFileRequest {
    project_id: String,
    file: String,
    name: String,
}

#[tauri::command]
pub fn list_projects(runtime: State<'_, AppRuntime>) -> Vec<ProjectSummary> {
    runtime.list()
}

#[tauri::command]
pub fn get_last_selected_project_id(runtime: State<'_, AppRuntime>) -> Option<String> {
    runtime.last_selected_project_id()
}

#[tauri::command]
pub fn set_last_selected_project(
    request: SelectedProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    runtime
        .remember_selected_project(request.project_id.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
pub fn register_project(
    root: String,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<ProjectSummary> {
    runtime.register(Path::new(&root)).map_err(Into::into)
}

#[tauri::command]
pub fn remove_project(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    credentials.revoke_project(&request.project_id)?;
    runtime.remove(&request.project_id).map_err(Into::into)
}

#[tauri::command]
pub fn rename_project(
    request: RenameProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<ProjectSummary> {
    runtime
        .rename_project(&request.project_id, &request.name)
        .map_err(Into::into)
}

#[tauri::command]
pub fn rename_env_file(
    request: RenameFileRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    runtime
        .rename_file(&request.project_id, &request.file, &request.name)
        .map_err(Into::into)
}

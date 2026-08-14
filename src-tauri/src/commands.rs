use std::path::Path;
use std::time::Duration;

use env_core::{
    AddVariableRequest, CodexAccess, CreateGroupRequest, DeleteVariableRequest, EnvError,
    GitignoreUpdateSummary, LinkRequest, MoveVariableRequest, MutationSummary, ProjectProjection,
    RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{AgentActivityEvent, AppRuntime, MigrationPlanProjection, ProjectSummary};
use crate::{
    integrations,
    integrations::AgentIntegrationId,
    provider_push::{self, ProviderPushRequest},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    code: String,
    message: String,
}

impl From<EnvError> for CommandError {
    fn from(error: EnvError) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
        }
    }
}

impl From<integrations::IntegrationError> for CommandError {
    fn from(error: integrations::IntegrationError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

impl From<provider_push::ProviderPushError> for CommandError {
    fn from(error: provider_push::ProviderPushError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

type CommandResult<T> = Result<T, CommandError>;

fn provider_task_interrupted() -> CommandError {
    CommandError {
        code: "PROVIDER_TASK_INTERRUPTED".to_owned(),
        message: "배포 서비스 확인 작업이 중단되었습니다.".to_owned(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRequest {
    project_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryRequest {
    project_id: String,
    repository: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareTargetRequest {
    project_id: String,
    file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGitHubEnvironmentRequest {
    project_id: String,
    repository: String,
    environment: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMutation<T> {
    project_id: String,
    request: T,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    project_id: String,
    passphrase: Option<String>,
    selection: Option<Vec<env_core::ExportOccurrence>>,
    locale: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    file_count: usize,
    encrypted: bool,
    cancelled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportRequest {
    project_id: String,
    passphrase: String,
    locale: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTeamImportRequest {
    project_id: String,
    plan_id: String,
    shared_conflicts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemapTeamImportFileRequest {
    project_id: String,
    plan_id: String,
    source_file: String,
    target_file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealTeamImportConflictRequest {
    project_id: String,
    plan_id: String,
    occurrence_id: String,
    side: env_core::TeamImportValueSide,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardTeamImportRequest {
    project_id: String,
    plan_id: String,
}

#[tauri::command]
pub fn list_projects(runtime: State<'_, AppRuntime>) -> Vec<ProjectSummary> {
    runtime.list()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationRequest {
    id: AgentIntegrationId,
}

#[tauri::command]
pub fn list_agent_integrations(app: AppHandle) -> Vec<integrations::AgentIntegrationStatus> {
    integrations::list(&app)
}

#[tauri::command]
pub async fn list_deployment_providers(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<provider_push::DeploymentProviderStatus>> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || provider_push::list(&root))
        .await
        .map_err(|_| provider_task_interrupted())
}

#[tauri::command]
pub async fn list_github_repositories(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubRepositoryOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || provider_push::list_github_repositories(&root))
        .await
        .map_err(|_| provider_task_interrupted())?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn detect_github_repository(
    request: CloudflareTargetRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubRepositoryContext> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let file = request.file;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::detect_github_repository(&root, &file)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn detect_cloudflare_target(
    request: CloudflareTargetRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::CloudflareTargetContext> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let file = request.file;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::detect_cloudflare_target(&root, &file)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn list_github_environments(
    request: GitHubRepositoryRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubEnvironmentOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let repository = request.repository;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::list_github_environments(&root, &repository)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn create_github_environment(
    request: CreateGitHubEnvironmentRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubEnvironmentOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let repository = request.repository;
    let environment = request.environment;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::create_github_environment(&root, &repository, &environment)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn push_to_provider(
    payload: ProjectMutation<ProviderPushRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::ProviderPushResult> {
    let service = runtime.service(&payload.project_id)?;
    tauri::async_runtime::spawn_blocking(move || provider_push::push(&service, payload.request))
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
) -> CommandResult<()> {
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

#[tauri::command]
pub async fn export_env_files(
    request: ExportRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<ExportResult> {
    let encrypted = request.passphrase.is_some();
    let korean = request.locale == "ko";
    if request
        .passphrase
        .as_ref()
        .is_some_and(|passphrase| passphrase.chars().count() < 10)
    {
        return Err(EnvError::invalid("암호는 10자 이상이어야 합니다.").into());
    }
    let project = runtime
        .list()
        .into_iter()
        .find(|project| project.id == request.project_id)
        .ok_or_else(|| EnvError::unregistered_project(&request.project_id))?;
    let safe_name = project
        .name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let extension = if encrypted { "zip.age" } else { "zip" };
    let dialog = app
        .dialog()
        .file()
        .set_title(if encrypted {
            if korean {
                "암호화 env 내보내기"
            } else {
                "Export encrypted env files"
            }
        } else {
            if korean {
                "env 내보내기"
            } else {
                "Export env files"
            }
        })
        .set_file_name(format!("{safe_name}-env.{extension}"))
        .add_filter(
            if encrypted {
                "age encrypted ZIP"
            } else {
                "ZIP archive"
            },
            &[if encrypted { "age" } else { "zip" }],
        );
    let Some(destination) = dialog.blocking_save_file() else {
        return Ok(ExportResult {
            file_count: 0,
            encrypted,
            cancelled: true,
        });
    };
    let destination = destination
        .into_path()
        .map_err(|_| EnvError::invalid("선택한 내보내기 경로를 사용할 수 없습니다."))?;
    let valid_extension = if encrypted {
        destination
            .extension()
            .is_some_and(|extension| extension == "age")
    } else {
        destination
            .extension()
            .is_some_and(|extension| extension == "zip")
    };
    if !valid_extension {
        return Err(EnvError::invalid(if encrypted {
            "암호화 내보내기 파일은 .age 확장자여야 합니다."
        } else {
            "일반 내보내기 파일은 .zip 확장자여야 합니다."
        })
        .into());
    }
    let service = runtime.service(&request.project_id)?;
    let passphrase = request.passphrase;
    let selection = request.selection;
    let summary = tauri::async_runtime::spawn_blocking(move || {
        service.export_env_files(&destination, passphrase, selection.as_deref())
    })
    .await
    .map_err(|_| EnvError::invalid("내보내기 작업이 중단되었습니다."))??;
    Ok(ExportResult {
        file_count: summary.file_count,
        encrypted,
        cancelled: false,
    })
}

#[tauri::command]
pub async fn plan_team_import(
    request: TeamImportRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Option<crate::runtime::TeamImportPlanProjection>> {
    if request.passphrase.chars().count() < 10 {
        return Err(EnvError::invalid("암호는 10자 이상이어야 합니다.").into());
    }
    let package = app
        .dialog()
        .file()
        .set_title(if request.locale == "ko" {
            "암호화 env 공유 파일 열기"
        } else {
            "Open encrypted env share"
        })
        .add_filter("age encrypted ZIP", &["age"])
        .blocking_pick_file();
    let Some(package) = package else {
        return Ok(None);
    };
    let package = package
        .into_path()
        .map_err(|_| EnvError::invalid("선택한 공유 파일을 사용할 수 없습니다."))?;
    let project_id = request.project_id;
    let passphrase = age::secrecy::SecretString::from(request.passphrase);
    let runtime_handle = runtime.inner();
    let plan = runtime_handle.plan_team_import(&project_id, &package, passphrase)?;
    Ok(Some(plan))
}

#[tauri::command]
pub fn apply_team_import(
    request: ApplyTeamImportRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<env_core::TeamImportSummary> {
    runtime
        .apply_team_import(
            &request.project_id,
            &request.plan_id,
            &request.shared_conflicts,
        )
        .map_err(Into::into)
}

#[tauri::command]
pub fn remap_team_import_file(
    request: RemapTeamImportFileRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<env_core::TeamImportPreview> {
    runtime
        .remap_team_import_file(
            &request.project_id,
            &request.plan_id,
            &request.source_file,
            &request.target_file,
        )
        .map_err(Into::into)
}

#[tauri::command]
pub fn reveal_team_import_conflict(
    request: RevealTeamImportConflictRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<String> {
    runtime
        .reveal_team_import_conflict(
            &request.project_id,
            &request.plan_id,
            &request.occurrence_id,
            request.side,
        )
        .map_err(Into::into)
}

#[tauri::command]
pub fn discard_team_import(request: DiscardTeamImportRequest, runtime: State<'_, AppRuntime>) {
    runtime.discard_team_import(&request.project_id, &request.plan_id);
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

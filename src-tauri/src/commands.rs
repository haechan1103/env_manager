use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use env_core::{
    AddVariableRequest, CodexAccess, CreateGroupRequest, DeleteVariableRequest, EnvError,
    GitignoreUpdateSummary, LinkRequest, MoveVariableRequest, MutationSummary, ProjectProjection,
    RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
};
use env_credentials::{
    AccountProjection, AccountSecretField, CreateAccountInput, CredentialError, UpdateAccountInput,
};
use env_provider::action_pack::{self, ActionExecutionRequest};
use env_provider::provider_push::{self, ProviderCompareRequest, ProviderPushRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{
    AgentActivityEvent, AppRuntime, CredentialRuntime, MigrationPlanProjection, ProjectSummary,
    ProviderPushReceipt, TeamChannelProjection,
};
use crate::{integrations, integrations::AgentIntegrationId};

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

impl From<action_pack::ActionPackError> for CommandError {
    fn from(error: action_pack::ActionPackError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

impl From<CredentialError> for CommandError {
    fn from(error: CredentialError) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
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
pub struct SelectedProjectRequest {
    project_id: Option<String>,
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
pub struct CloudflareAccessRequest {
    project_id: String,
    file: String,
    worker: String,
    environment: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EasAccessRequest {
    project_id: String,
    file: String,
    project: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsAccessRequest {
    profile: Option<String>,
    region: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRuntimeTargetRequest {
    project_id: String,
    target_id: String,
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
pub struct CreateAccountRequest {
    project_id: String,
    display_name: String,
    service: String,
    username: String,
    password: String,
    allow_current_project: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountRequest {
    project_id: String,
    account_id: String,
    display_name: String,
    service: String,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRequest {
    project_id: String,
    account_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountAccessRequest {
    project_id: String,
    account_id: String,
    allowed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyAccountFieldRequest {
    project_id: String,
    account_id: String,
    field: AccountField,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountField {
    Username,
    Password,
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
pub struct TeamChannelRequest {
    project_id: String,
    channel_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectFolderTeamChannelRequest {
    project_id: String,
    locale: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishTeamChannelRequest {
    project_id: String,
    channel_id: String,
    passphrase: String,
    selection: Option<Vec<env_core::ExportOccurrence>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTeamChannelImportRequest {
    project_id: String,
    channel_id: String,
    package_id: String,
    passphrase: String,
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
pub async fn list_deployment_providers(
    request: ProjectRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<provider_push::DeploymentProviderStatus>> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || provider_push::list(&root, &app_data))
        .await
        .map_err(|_| provider_task_interrupted())
}

#[tauri::command]
pub async fn list_github_repositories(
    request: ProjectRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubRepositoryOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::list_github_repositories(&root, &app_data)
    })
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
pub async fn detect_eas_target(
    request: CloudflareTargetRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::EasTargetContext> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let file = request.file;
    tauri::async_runtime::spawn_blocking(move || provider_push::detect_eas_target(&root, &file))
        .await
        .map_err(|_| provider_task_interrupted())?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn inspect_eas_access(
    request: EasAccessRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::EasAccessContext> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::inspect_eas_access(
            &root,
            &app_data,
            &request.file,
            request.project.as_deref(),
        )
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn inspect_cloudflare_access(
    request: CloudflareAccessRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::CloudflareAccessContext> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::inspect_cloudflare_access(
            &root,
            &app_data,
            &request.file,
            &request.worker,
            request.environment.as_deref(),
        )
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn inspect_aws_access(
    request: AwsAccessRequest,
) -> CommandResult<provider_push::AwsAccessContext> {
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::inspect_aws_access(request.profile.as_deref(), request.region.as_deref())
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn list_github_environments(
    request: GitHubRepositoryRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubEnvironmentOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    let repository = request.repository;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::list_github_environments(&root, &app_data, &repository)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn create_github_environment(
    request: CreateGitHubEnvironmentRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::GitHubEnvironmentOptions> {
    let service = runtime.service(&request.project_id)?;
    let root = service.root().to_path_buf();
    let app_data = provider_app_data(&app)?;
    let repository = request.repository;
    let environment = request.environment;
    tauri::async_runtime::spawn_blocking(move || {
        provider_push::create_github_environment(&root, &app_data, &repository, &environment)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(Into::into)
}

#[tauri::command]
pub async fn push_to_provider(
    payload: ProjectMutation<ProviderPushRequest>,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::ProviderPushResult> {
    let project_id = payload.project_id;
    let service = runtime.service(&project_id)?;
    let app_data = provider_app_data(&app)?;
    let receipt_request = payload.request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        provider_push::push(&service, &app_data, payload.request)
    })
    .await
    .map_err(|_| provider_task_interrupted())?
    .map_err(CommandError::from)?;
    runtime.record_provider_push(provider_push_receipt(project_id, &receipt_request, &result))?;
    Ok(result)
}

#[tauri::command]
pub fn list_provider_push_receipts(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<ProviderPushReceipt>> {
    runtime
        .provider_push_receipts(&request.project_id)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn compare_provider_values(
    payload: ProjectMutation<ProviderCompareRequest>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<provider_push::ProviderCompareResult> {
    let service = runtime.service(&payload.project_id)?;
    tauri::async_runtime::spawn_blocking(move || provider_push::compare(&service, payload.request))
        .await
        .map_err(|_| provider_task_interrupted())?
        .map_err(Into::into)
}

#[tauri::command]
pub fn list_runtime_targets(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<env_provider::runtime_target::RuntimeTarget>> {
    let service = runtime.service(&request.project_id)?;
    env_provider::runtime_target::list(service.root()).map_err(Into::into)
}

#[tauri::command]
pub fn save_runtime_target(
    payload: ProjectMutation<env_provider::runtime_target::RuntimeTarget>,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<env_provider::runtime_target::RuntimeTarget>> {
    let service = runtime.service(&payload.project_id)?;
    env_provider::runtime_target::save(service.root(), payload.request).map_err(Into::into)
}

#[tauri::command]
pub fn remove_runtime_target(
    request: RemoveRuntimeTargetRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<env_provider::runtime_target::RuntimeTarget>> {
    let service = runtime.service(&request.project_id)?;
    env_provider::runtime_target::remove(service.root(), &request.target_id).map_err(Into::into)
}

fn provider_app_data(app: &AppHandle) -> CommandResult<std::path::PathBuf> {
    app.path().app_data_dir().map_err(|_| CommandError {
        code: "PROVIDER_ADAPTER_STORAGE_UNAVAILABLE".to_owned(),
        message: "Provider Adapter 저장 위치를 확인하지 못했습니다.".to_owned(),
    })
}

fn provider_push_receipt(
    project_id: String,
    request: &ProviderPushRequest,
    result: &provider_push::ProviderPushResult,
) -> ProviderPushReceipt {
    let failed = result
        .failed_keys
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let succeeded_keys = request
        .selections
        .iter()
        .filter(|selection| !failed.contains(&selection.key))
        .map(|selection| selection.key.clone())
        .collect();
    let timestamp_ms = if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
        duration.as_millis().min(u128::from(u64::MAX)) as u64
    } else {
        0
    };
    ProviderPushReceipt {
        timestamp_ms,
        project_id,
        provider: request.provider.clone(),
        source_file: request.file.clone(),
        destination: provider_destination(request),
        succeeded_keys,
        failed_keys: result.failed_keys.clone(),
    }
}

fn provider_destination(request: &ProviderPushRequest) -> String {
    match request.provider.as_str() {
        "github-actions" => match (&request.repository, &request.github_environment) {
            (Some(repository), Some(environment)) => format!("{repository} · {environment}"),
            (Some(repository), None) => repository.clone(),
            _ => "github-actions".to_owned(),
        },
        "cloudflare-workers" => match (&request.worker, &request.cloudflare_environment) {
            (Some(worker), Some(environment)) => format!("{worker} · {environment}"),
            (Some(worker), None) => worker.clone(),
            _ => "cloudflare-workers".to_owned(),
        },
        "expo-eas" => match &request.eas_project {
            Some(project) => format!("{} · {}", project, request.eas_environments.join(", ")),
            None => "expo-eas".to_owned(),
        },
        "aws-secrets-manager" | "aws-ssm-parameter-store" => {
            let region = request.aws_region.as_deref().unwrap_or("default-region");
            match request.aws_path_prefix.as_deref() {
                Some(prefix) if !prefix.is_empty() => format!("{region}/{prefix}"),
                _ => region.to_owned(),
            }
        }
        _ => request
            .personal_target
            .clone()
            .unwrap_or_else(|| request.provider.clone()),
    }
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
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    credentials.revoke_project(&request.project_id)?;
    runtime.remove(&request.project_id).map_err(Into::into)
}

#[tauri::command]
pub fn list_accounts(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<Vec<AccountProjection>> {
    let _ = runtime.service(&request.project_id)?;
    credentials.list(&request.project_id).map_err(Into::into)
}

#[tauri::command]
pub fn create_account(
    request: CreateAccountRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<AccountProjection> {
    let _ = runtime.service(&request.project_id)?;
    credentials
        .create(CreateAccountInput {
            display_name: request.display_name,
            service: request.service,
            username: zeroize::Zeroizing::new(request.username),
            password: zeroize::Zeroizing::new(request.password),
            grant_project_id: request.allow_current_project.then_some(request.project_id),
        })
        .map_err(Into::into)
}

#[tauri::command]
pub fn update_account(
    request: UpdateAccountRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    let _ = runtime.service(&request.project_id)?;
    credentials
        .update(UpdateAccountInput {
            account_id: request.account_id,
            display_name: request.display_name,
            service: request.service,
            username: request.username.map(zeroize::Zeroizing::new),
            password: request.password.map(zeroize::Zeroizing::new),
        })
        .map_err(Into::into)
}

#[tauri::command]
pub fn delete_account(
    request: AccountRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    let _ = runtime.service(&request.project_id)?;
    credentials.delete(&request.account_id).map_err(Into::into)
}

#[tauri::command]
pub fn set_account_project_access(
    request: AccountAccessRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    let _ = runtime.service(&request.project_id)?;
    credentials
        .set_project_access(&request.account_id, &request.project_id, request.allowed)
        .map_err(Into::into)
}

#[tauri::command]
pub fn copy_account_field(
    request: CopyAccountFieldRequest,
    runtime: State<'_, AppRuntime>,
    credentials: State<'_, CredentialRuntime>,
) -> CommandResult<()> {
    let _ = runtime.service(&request.project_id)?;
    let field = match request.field {
        AccountField::Username => AccountSecretField::Username,
        AccountField::Password => AccountSecretField::Password,
    };
    let value = credentials.secret_field(&request.account_id, &request.project_id, field)?;
    let mut clipboard = arboard::Clipboard::new().map_err(|_| CommandError {
        code: "CLIPBOARD_UNAVAILABLE".to_owned(),
        message: "클립보드를 사용할 수 없습니다.".to_owned(),
    })?;
    clipboard
        .set_text(value.as_str())
        .map_err(|_| CommandError {
            code: "CLIPBOARD_WRITE_FAILED".to_owned(),
            message: "계정 정보를 클립보드에 복사하지 못했습니다.".to_owned(),
        })?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(45));
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && clipboard
                .get_text()
                .is_ok_and(|current| current == value.as_str())
        {
            let _ = clipboard.set_text(String::new());
        }
    });
    Ok(())
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
pub fn list_team_channels(
    request: ProjectRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Vec<TeamChannelProjection>> {
    runtime
        .list_team_channels(&request.project_id)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn connect_folder_team_channel(
    request: ConnectFolderTeamChannelRequest,
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<Option<TeamChannelProjection>> {
    let selected = app
        .dialog()
        .file()
        .set_title(if request.locale == "ko" {
            "팀 공유 폴더 선택"
        } else {
            "Choose a team share folder"
        })
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| EnvError::invalid("선택한 팀 공유 폴더를 사용할 수 없습니다."))?;
    let folder_name = path.file_name().map_or_else(
        || "Team channel".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let suggested_name = folder_name
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect::<String>();
    let suggested_name = if suggested_name.trim().is_empty() {
        "Team channel".to_owned()
    } else {
        suggested_name
    };
    runtime
        .connect_folder_team_channel(&request.project_id, &path, &suggested_name)
        .map(Some)
        .map_err(Into::into)
}

#[tauri::command]
pub fn remove_team_channel(
    request: TeamChannelRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<()> {
    runtime
        .remove_team_channel(&request.project_id, &request.channel_id)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn publish_team_channel(
    request: PublishTeamChannelRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<env_team::TeamPackagePublishSummary> {
    if request.passphrase.chars().count() < 10 {
        return Err(EnvError::invalid("암호는 10자 이상이어야 합니다.").into());
    }
    let (service, transport) =
        runtime.prepare_team_channel_operation(&request.project_id, &request.channel_id)?;
    let passphrase = age::secrecy::SecretString::from(request.passphrase);
    let selection = request.selection;
    let summary = tauri::async_runtime::spawn_blocking(move || {
        env_team::publish_project_package(
            &service,
            transport.as_ref(),
            passphrase,
            selection.as_deref(),
        )
    })
    .await
    .map_err(|_| EnvError::invalid("팀 채널 게시 작업이 중단되었습니다."))??;
    Ok(summary)
}

#[tauri::command]
pub fn plan_team_channel_import(
    request: PlanTeamChannelImportRequest,
    runtime: State<'_, AppRuntime>,
) -> CommandResult<crate::runtime::TeamImportPlanProjection> {
    if request.passphrase.chars().count() < 10 {
        return Err(EnvError::invalid("암호는 10자 이상이어야 합니다.").into());
    }
    runtime
        .plan_team_channel_import(
            &request.project_id,
            &request.channel_id,
            &request.package_id,
            age::secrecy::SecretString::from(request.passphrase),
        )
        .map_err(Into::into)
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

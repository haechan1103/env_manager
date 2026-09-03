use super::*;

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

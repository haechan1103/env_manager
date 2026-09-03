use super::*;

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

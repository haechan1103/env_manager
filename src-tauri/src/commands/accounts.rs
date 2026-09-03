use super::*;

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

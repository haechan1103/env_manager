use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use env_core::{EnvError, PreparedOpaqueValueWrite, ProjectService};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::append_audit_event;

const PLAN_SCHEMA: &str = "env-manager-stdin-value-plan-v1";
const PLAN_TTL_MS: u64 = 5 * 60 * 1_000;
const MAX_STDIN_BYTES: u64 = 64 * 1024;
const PLAN_DIRECTORY: &str = "stdin-value-plans";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StdinValuePlanProjection {
    pub plan_id: String,
    pub project_id: String,
    pub affected_files: Vec<String>,
    pub keys: Vec<String>,
    pub broker_executable: String,
    pub trim_final_newline: bool,
    pub expires_in_seconds: u64,
    pub risk: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StdinValueApplyProjection {
    pub affected_files: Vec<String>,
    pub keys: Vec<String>,
    pub result_code: &'static str,
}

#[derive(Debug)]
pub struct StdinValueError {
    code: &'static str,
    message: String,
}

impl StdinValueError {
    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn sanitized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }
}

impl From<EnvError> for StdinValueError {
    fn from(error: EnvError) -> Self {
        Self::new(error.code().as_str(), error.to_string())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredStdinValuePlan {
    schema: String,
    plan_id: String,
    created_at_ms: u64,
    expires_at_ms: u64,
    actor: String,
    trim_final_newline: bool,
    prepared: PreparedOpaqueValueWrite,
}

pub fn create_plan(
    app_data: &Path,
    service: &ProjectService,
    file: &str,
    key: &str,
    trim_final_newline: bool,
    actor: &str,
    broker_executable: &Path,
) -> Result<StdinValuePlanProjection, EnvError> {
    let prepared = service.prepare_opaque_value_write(file, key)?;
    let directory = plan_directory(app_data);
    fs::create_dir_all(&directory).map_err(|error| EnvError::io(&directory, error))?;
    set_private_directory_permissions(&directory)?;
    cleanup_expired_plans(&directory);

    let now_ms = unix_time_ms();
    for _ in 0..4 {
        let plan_id = random_plan_id()?;
        let stored = StoredStdinValuePlan {
            schema: PLAN_SCHEMA.to_owned(),
            plan_id: plan_id.clone(),
            created_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(PLAN_TTL_MS),
            actor: actor.to_owned(),
            trim_final_newline,
            prepared: prepared.clone(),
        };
        let path = plan_path(&directory, &plan_id);
        match open_private_plan_file(&path) {
            Ok(mut output) => {
                if let Err(error) = serde_json::to_writer(&mut output, &stored)
                    .map_err(EnvError::serialization)
                    .and_then(|()| {
                        output
                            .write_all(b"\n")
                            .map_err(|error| EnvError::io(&path, error))
                    })
                    .and_then(|()| {
                        output
                            .sync_all()
                            .map_err(|error| EnvError::io(&path, error))
                    })
                {
                    drop(output);
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(StdinValuePlanProjection {
                    plan_id,
                    project_id: stored.prepared.project_id.clone(),
                    affected_files: stored.prepared.affected_files.clone(),
                    keys: vec![stored.prepared.key.clone()],
                    broker_executable: broker_executable.to_string_lossy().into_owned(),
                    trim_final_newline,
                    expires_in_seconds: PLAN_TTL_MS / 1_000,
                    risk: "opaque-stdin-value-write",
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(EnvError::io(&path, error)),
        }
    }
    Err(EnvError::invalid(
        "일회용 stdin 값 계획 ID를 만들지 못했습니다.",
    ))
}

pub fn apply_plan<R: Read>(
    app_data: &Path,
    registry_path: &Path,
    plan_id: &str,
    trim_final_newline: bool,
    reader: R,
) -> Result<StdinValueApplyProjection, StdinValueError> {
    validate_plan_id(plan_id)?;
    let directory = plan_directory(app_data);
    let source = plan_path(&directory, plan_id);
    let claimed = claimed_plan_path(&directory, plan_id);
    fs::rename(&source, &claimed).map_err(|_| plan_unavailable())?;

    let result = apply_claimed_plan(
        app_data,
        registry_path,
        plan_id,
        trim_final_newline,
        reader,
        &claimed,
    );
    let _ = fs::remove_file(&claimed);
    result
}

fn apply_claimed_plan<R: Read>(
    app_data: &Path,
    registry_path: &Path,
    plan_id: &str,
    trim_final_newline: bool,
    reader: R,
    claimed_path: &Path,
) -> Result<StdinValueApplyProjection, StdinValueError> {
    let plan_bytes = fs::read(claimed_path).map_err(|_| plan_unavailable())?;
    let stored: StoredStdinValuePlan =
        serde_json::from_slice(&plan_bytes).map_err(|_| plan_unavailable())?;
    if stored.schema != PLAN_SCHEMA || stored.plan_id != plan_id {
        return Err(plan_unavailable());
    }
    let now_ms = unix_time_ms();
    if stored.expires_at_ms != stored.created_at_ms.saturating_add(PLAN_TTL_MS)
        || now_ms < stored.created_at_ms
        || stored.expires_at_ms < now_ms
    {
        return audit_failure(app_data, &stored, plan_unavailable());
    }
    if stored.trim_final_newline != trim_final_newline {
        return audit_failure(
            app_data,
            &stored,
            StdinValueError::new(
                "STDIN_NORMALIZATION_MISMATCH",
                "계획과 stdin 줄바꿈 처리 옵션이 일치하지 않습니다.",
            ),
        );
    }

    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(MAX_STDIN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StdinValueError::new("STDIN_READ_FAILED", "stdin 값을 읽지 못했습니다."))?;
    if bytes.len() as u64 > MAX_STDIN_BYTES {
        return audit_failure(
            app_data,
            &stored,
            StdinValueError::new(
                "STDIN_VALUE_TOO_LARGE",
                "stdin 값이 64 KiB 제한을 초과했습니다.",
            ),
        );
    }
    if bytes.contains(&0) {
        return audit_failure(
            app_data,
            &stored,
            StdinValueError::new("STDIN_VALUE_INVALID", "stdin 값에 NUL 문자가 있습니다."),
        );
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => {
            return audit_failure(
                app_data,
                &stored,
                StdinValueError::new("STDIN_VALUE_INVALID_UTF8", "stdin 값은 UTF-8이어야 합니다."),
            );
        }
    };
    let mut value = Zeroizing::new(text.to_owned());
    if trim_final_newline && value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    if value.is_empty() {
        return audit_failure(
            app_data,
            &stored,
            StdinValueError::new("STDIN_VALUE_EMPTY", "빈 stdin 값은 저장할 수 없습니다."),
        );
    }

    let registry = match env_registry::read(registry_path) {
        Ok(registry) => registry,
        Err(error) => return audit_failure(app_data, &stored, error.into()),
    };
    let registration = match registry
        .projects
        .into_iter()
        .find(|project| project.id == stored.prepared.project_id)
    {
        Some(registration) => registration,
        None => {
            return audit_failure(
                app_data,
                &stored,
                StdinValueError::new(
                    "UNREGISTERED_PROJECT",
                    "계획의 프로젝트가 더 이상 등록되어 있지 않습니다.",
                ),
            );
        }
    };
    let service = match ProjectService::open(&registration.root) {
        Ok(service) => service,
        Err(error) => return audit_failure(app_data, &stored, error.into()),
    };
    if service.project_id() != stored.prepared.project_id
        || !service.root().join(env_core::MANIFEST_FILE_NAME).is_file()
    {
        return audit_failure(
            app_data,
            &stored,
            StdinValueError::new(
                "UNREGISTERED_PROJECT",
                "계획의 프로젝트 등록 상태가 변경되었습니다.",
            ),
        );
    }
    let mutation = service
        .apply_prepared_opaque_value(&stored.prepared, &value)
        .map_err(StdinValueError::from);
    match mutation {
        Ok(summary) => {
            append_audit_event(
                Some(app_data),
                &stored.prepared.project_id,
                stored.actor,
                "apply_stdin_value",
                &summary.affected_files,
                &summary.keys,
                "opaque-stdin-value-write",
                "OK",
            );
            Ok(StdinValueApplyProjection {
                affected_files: summary.affected_files,
                keys: summary.keys,
                result_code: "OK",
            })
        }
        Err(error) => audit_failure(app_data, &stored, error),
    }
}

fn audit_failure<T>(
    app_data: &Path,
    stored: &StoredStdinValuePlan,
    error: StdinValueError,
) -> Result<T, StdinValueError> {
    append_audit_event(
        Some(app_data),
        &stored.prepared.project_id,
        stored.actor.clone(),
        "apply_stdin_value",
        &stored.prepared.affected_files,
        std::slice::from_ref(&stored.prepared.key),
        "opaque-stdin-value-write",
        error.code(),
    );
    Err(error)
}

fn plan_directory(app_data: &Path) -> PathBuf {
    app_data.join(PLAN_DIRECTORY)
}

fn plan_path(directory: &Path, plan_id: &str) -> PathBuf {
    directory.join(format!("{plan_id}.json"))
}

fn claimed_plan_path(directory: &Path, plan_id: &str) -> PathBuf {
    directory.join(format!("{plan_id}.claimed"))
}

fn validate_plan_id(plan_id: &str) -> Result<(), StdinValueError> {
    let token = plan_id
        .strip_prefix("stdin-plan-")
        .ok_or_else(plan_unavailable)?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(plan_unavailable());
    }
    Ok(())
}

fn random_plan_id() -> Result<String, EnvError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| EnvError::invalid("안전한 일회용 계획 ID를 만들지 못했습니다."))?;
    let mut id = String::with_capacity("stdin-plan-".len() + bytes.len() * 2);
    id.push_str("stdin-plan-");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(id, "{byte:02x}");
    }
    Ok(id)
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn plan_unavailable() -> StdinValueError {
    StdinValueError::new(
        "PLAN_EXPIRED",
        "일회용 stdin 값 계획이 없거나 만료되었거나 이미 사용되었습니다.",
    )
}

fn cleanup_expired_plans(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.filter_map(Result::ok).take(128) {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(plan) = serde_json::from_slice::<StoredStdinValuePlan>(&bytes) else {
            continue;
        };
        if plan.schema == PLAN_SCHEMA && plan.expires_at_ms < unix_time_ms() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), EnvError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| EnvError::io(path, error))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), EnvError> {
    Ok(())
}

#[cfg(unix)]
fn open_private_plan_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_plan_file(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

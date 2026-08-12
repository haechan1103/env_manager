use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use env_core::{
    AddVariableRequest, ClassificationSource, CodexAccess, CreateEnvFileRequest,
    CreateGroupRequest, EnvError, EnvErrorCode, LinkRequest, MigrationPlan, MoveVariableRequest,
    ProjectService, RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PLAN_TTL: Duration = Duration::from_secs(300);

pub struct Broker {
    plans: Mutex<HashMap<String, StoredPlan>>,
    next_plan_id: AtomicU64,
    registered_roots_override: Option<Vec<PathBuf>>,
}

impl Default for Broker {
    fn default() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
            registered_roots_override: None,
        }
    }
}

struct StoredPlan {
    project_id: String,
    root: PathBuf,
    expires_at: Instant,
    operation: PlannedOperation,
    affected_files: Vec<String>,
    keys: Vec<String>,
    risk: &'static str,
}

enum PlannedOperation {
    SetAllowedValue(SaveValueRequest),
    CreateEnvFile(CreateEnvFileRequest),
    AddVariable(AddVariableRequest),
    CreateGroup(CreateGroupRequest),
    RenameGroup(RenameGroupRequest),
    MoveVariable(MoveVariableRequest),
    UpdateDescription(SaveDescriptionRequest),
    Link(LinkRequest),
    Detach { link_id: String, file: String },
    Classification { key: String, access: CodexAccess },
    Migration(MigrationPlan),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanProjection {
    plan_id: String,
    project_id: String,
    summary: String,
    affected_files: Vec<String>,
    keys: Vec<String>,
    risk: &'static str,
    expires_in_seconds: u64,
    migration: Option<env_core::MigrationPreview>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    project_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValueArgs {
    project_path: String,
    file: String,
    key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanValueArgs {
    project_path: String,
    file: String,
    key: String,
    new_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanCreateEnvFileArgs {
    project_path: String,
    file: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanAddVariableArgs {
    project_path: String,
    file: String,
    key: String,
    group: String,
    #[serde(default)]
    description: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanCreateGroupArgs {
    project_path: String,
    file: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanRenameGroupArgs {
    project_path: String,
    file: String,
    current_name: String,
    new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanMoveVariableArgs {
    project_path: String,
    file: String,
    key: String,
    target_group: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanDescriptionArgs {
    project_path: String,
    file: String,
    key: String,
    lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanLinkArgs {
    project_path: String,
    key: String,
    files: Vec<String>,
    source_file: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanDetachArgs {
    project_path: String,
    link_id: String,
    file: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanClassificationArgs {
    project_path: String,
    key: String,
    access: CodexAccess,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanMigrationArgs {
    project_path: String,
    file: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyArgs {
    plan_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditEvent<'a> {
    timestamp_ms: u64,
    project_id: &'a str,
    actor: String,
    category: &'static str,
    operation: &'a str,
    relative_paths: &'a [String],
    variable_names: &'a [String],
    policy_decision: &'a str,
    outcome: &'static str,
    result_code: &'a str,
}

impl Broker {
    pub fn with_registered_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            registered_roots_override: Some(roots),
            ..Self::default()
        }
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, EnvError> {
        match name {
            "inspect_project" => self.inspect(parse(arguments)?),
            "read_allowed_value" => self.read_allowed(parse(arguments)?),
            "plan_set_allowed_value" => self.plan_value(parse(arguments)?),
            "plan_create_env_file" => self.plan_create_env_file(parse(arguments)?),
            "plan_add_variable" => self.plan_add_variable(parse(arguments)?),
            "plan_create_group" => self.plan_create_group(parse(arguments)?),
            "plan_rename_group" => self.plan_rename_group(parse(arguments)?),
            "plan_move_variable" => self.plan_move_variable(parse(arguments)?),
            "plan_update_description" => self.plan_update_description(parse(arguments)?),
            "plan_link" => self.plan_link(parse(arguments)?),
            "plan_detach" => self.plan_detach(parse(arguments)?),
            "plan_classification" => self.plan_classification(parse(arguments)?),
            "plan_migration" => self.plan_migration(parse(arguments)?),
            "apply_plan" => self.apply(parse(arguments)?),
            _ => Err(EnvError::invalid("지원하지 않는 Env Manager 도구입니다.")),
        }
    }

    fn inspect(&self, args: InspectArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let projection = service.scan()?;
        let result = serde_json::to_value(projection).map_err(EnvError::serialization)?;
        audit(
            service.project_id(),
            "inspect_project",
            &[],
            &[],
            "redacted",
            "OK",
        );
        Ok(result)
    }

    fn read_allowed(&self, args: ValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let value = service.read_allowed_value(&args.file, &args.key);
        let code = value
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        audit(
            service.project_id(),
            "read_allowed_value",
            std::slice::from_ref(&args.file),
            std::slice::from_ref(&args.key),
            "policy-checked",
            code,
        );
        Ok(json!({ "value": value? }))
    }

    fn plan_value(&self, args: PlanValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        if service.codex_access(&args.key)? != CodexAccess::ReadWrite {
            let error = EnvError::access_blocked(&args.key);
            audit(
                service.project_id(),
                "plan_set_allowed_value",
                std::slice::from_ref(&args.file),
                std::slice::from_ref(&args.key),
                "blocked-by-policy",
                error.code().as_str(),
            );
            return Err(error);
        }
        self.store_plan(
            &service,
            PlannedOperation::SetAllowedValue(SaveValueRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                new_value: args.new_value,
            }),
            format!("{}의 값을 정책 허용 범위에서 교체합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "value-write",
            None,
        )
    }

    fn plan_create_env_file(&self, args: PlanCreateEnvFileArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::CreateEnvFile(CreateEnvFileRequest {
                file: args.file.clone(),
            }),
            format!("{} 빈 env 파일을 만듭니다.", args.file),
            vec![args.file],
            Vec::new(),
            "file-create",
            None,
        )
    }

    fn plan_add_variable(&self, args: PlanAddVariableArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::AddVariable(AddVariableRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                group: args.group,
                description: args.description,
                value: String::new(),
            }),
            format!("{} 빈 변수를 추가합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    fn plan_create_group(&self, args: PlanCreateGroupArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::CreateGroup(CreateGroupRequest {
                file: args.file.clone(),
                name: args.name.clone(),
            }),
            format!("{} 그룹을 만듭니다.", args.name),
            vec![args.file],
            Vec::new(),
            "structural-write",
            None,
        )
    }

    fn plan_rename_group(&self, args: PlanRenameGroupArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::RenameGroup(RenameGroupRequest {
                file: args.file.clone(),
                current_name: args.current_name.clone(),
                new_name: args.new_name.clone(),
            }),
            format!(
                "{} 그룹 이름을 {}로 바꿉니다.",
                args.current_name, args.new_name
            ),
            vec![args.file],
            Vec::new(),
            "structural-write",
            None,
        )
    }

    fn plan_move_variable(&self, args: PlanMoveVariableArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::MoveVariable(MoveVariableRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                target_group: args.target_group.clone(),
            }),
            format!(
                "{} 변수를 {} 그룹으로 옮깁니다.",
                args.key, args.target_group
            ),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    fn plan_update_description(&self, args: PlanDescriptionArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::UpdateDescription(SaveDescriptionRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                lines: args.lines,
            }),
            format!("{} 변수 설명을 변경합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    fn plan_link(&self, args: PlanLinkArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Link(LinkRequest {
                key: args.key.clone(),
                files: args.files.clone(),
                source_file: args.source_file,
            }),
            format!(
                "{} occurrence {}개를 peer link로 연결합니다.",
                args.key,
                args.files.len()
            ),
            args.files,
            vec![args.key],
            "multi-file-write",
            None,
        )
    }

    fn plan_detach(&self, args: PlanDetachArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Detach {
                link_id: args.link_id,
                file: args.file.clone(),
            },
            "현재 occurrence를 연결에서 분리하고 값은 유지합니다.".to_owned(),
            vec![args.file],
            Vec::new(),
            "relationship-change",
            None,
        )
    }

    fn plan_classification(&self, args: PlanClassificationArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Classification {
                key: args.key.clone(),
                access: args.access,
            },
            format!("{}의 Codex 접근 정책을 변경합니다.", args.key),
            Vec::new(),
            vec![args.key],
            if args.access == CodexAccess::ReadWrite {
                "protection-downgrade"
            } else {
                "policy-change"
            },
            None,
        )
    }

    fn plan_migration(&self, args: PlanMigrationArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let migration = service.plan_migration(&args.file)?;
        let preview = migration.preview.clone();
        self.store_plan(
            &service,
            PlannedOperation::Migration(migration),
            preview.summary.clone(),
            vec![args.file],
            Vec::new(),
            "structural-write",
            Some(preview),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn store_plan(
        &self,
        service: &ProjectService,
        operation: PlannedOperation,
        summary: String,
        affected_files: Vec<String>,
        keys: Vec<String>,
        risk: &'static str,
        migration: Option<env_core::MigrationPreview>,
    ) -> Result<Value, EnvError> {
        let plan_id = format!(
            "plan-{}-{}",
            service.project_id(),
            self.next_plan_id.fetch_add(1, Ordering::Relaxed)
        );
        let projection = PlanProjection {
            plan_id: plan_id.clone(),
            project_id: service.project_id().to_owned(),
            summary,
            affected_files: affected_files.clone(),
            keys: keys.clone(),
            risk,
            expires_in_seconds: PLAN_TTL.as_secs(),
            migration,
        };
        self.plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                plan_id,
                StoredPlan {
                    project_id: service.project_id().to_owned(),
                    root: service.root().to_path_buf(),
                    expires_at: Instant::now() + PLAN_TTL,
                    operation,
                    affected_files: affected_files.clone(),
                    keys: keys.clone(),
                    risk,
                },
            );
        audit(
            service.project_id(),
            "create_plan",
            &affected_files,
            &keys,
            risk,
            "OK",
        );
        serde_json::to_value(projection).map_err(EnvError::serialization)
    }

    fn apply(&self, args: ApplyArgs) -> Result<Value, EnvError> {
        let stored = self
            .plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&args.plan_id)
            .ok_or_else(plan_expired)?;
        if stored.expires_at < Instant::now() {
            return Err(plan_expired());
        }

        let service = ProjectService::open(&stored.root)?;
        if service.project_id() != stored.project_id {
            return Err(EnvError::unregistered_project(&stored.project_id));
        }
        let affected_files = stored.affected_files.clone();
        let keys = stored.keys.clone();
        let risk = stored.risk;
        let result: Result<Value, EnvError> = match stored.operation {
            PlannedOperation::SetAllowedValue(request) => {
                if service.codex_access(&request.key)? != CodexAccess::ReadWrite {
                    let error = EnvError::access_blocked(&request.key);
                    audit(
                        service.project_id(),
                        "apply_plan",
                        &affected_files,
                        &keys,
                        "blocked-by-policy",
                        error.code().as_str(),
                    );
                    return Err(error);
                }
                serialize_result(service.save_value(request))
            }
            PlannedOperation::CreateEnvFile(request) => {
                serialize_result(service.create_env_file(request))
            }
            PlannedOperation::AddVariable(request) => {
                serialize_result(service.add_variable(request))
            }
            PlannedOperation::CreateGroup(request) => {
                serialize_result(service.create_group(request))
            }
            PlannedOperation::RenameGroup(request) => {
                serialize_result(service.rename_group(request))
            }
            PlannedOperation::MoveVariable(request) => {
                serialize_result(service.move_variable(request))
            }
            PlannedOperation::UpdateDescription(request) => {
                serialize_result(service.save_description(request))
            }
            PlannedOperation::Link(request) => serialize_result(service.create_link(request)),
            PlannedOperation::Detach { link_id, file } => service
                .detach_link_member(&link_id, &file)
                .map(|()| json!({ "affectedFiles": [file], "keys": [] })),
            PlannedOperation::Classification { key, access } => service
                .set_codex_access_by(&key, access, ClassificationSource::Codex)
                .map(|()| json!({ "affectedFiles": [], "keys": [key] })),
            PlannedOperation::Migration(plan) => serialize_result(service.apply_migration(plan)),
        };
        let result_code = result
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        audit(
            service.project_id(),
            "apply_plan",
            &affected_files,
            &keys,
            risk,
            result_code,
        );
        result
    }

    fn open_registered(&self, project_path: &str) -> Result<ProjectService, EnvError> {
        let path = Path::new(project_path);
        let root = path
            .canonicalize()
            .map_err(|error| EnvError::io(path, error))?;
        let registered_roots = self
            .registered_roots_override
            .clone()
            .map_or_else(load_registered_roots, Ok)?;
        let registered = registered_roots.into_iter().any(|candidate| {
            candidate
                .canonicalize()
                .is_ok_and(|candidate| candidate == root)
        });
        if !registered {
            return Err(EnvError::unregistered_project(
                "active-registration-required",
            ));
        }
        if !root.join(env_core::MANIFEST_FILE_NAME).is_file() {
            return Err(EnvError::unregistered_project("manifest-missing"));
        }
        ProjectService::open(root)
    }
}

fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, EnvError> {
    serde_json::from_value(value).map_err(|_| EnvError::invalid("도구 인자가 올바르지 않습니다."))
}

fn serialize_result<T: Serialize>(result: Result<T, EnvError>) -> Result<Value, EnvError> {
    result.and_then(|value| serde_json::to_value(value).map_err(EnvError::serialization))
}

/// Returns a Claude/Copilot-compatible PreToolUse decision without echoing tool input.
/// This is defense in depth; the broker remains the policy boundary for env operations.
pub fn guard_hook_decision(input: &Value) -> Value {
    if hook_requests_direct_env_access(input) {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "Direct .env access is blocked by Env Manager. Use the env-manager MCP tools instead."
            }
        });
    }
    json!({})
}

fn hook_requests_direct_env_access(input: &Value) -> bool {
    let tool_name = input
        .get("tool_name")
        .or_else(|| input.get("toolName"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tool_input = input
        .get("tool_input")
        .or_else(|| input.get("toolInput"))
        .unwrap_or(&Value::Null);

    if contains_env_path_field(tool_input) {
        return true;
    }

    let command_like = tool_name.contains("bash")
        || tool_name.contains("shell")
        || tool_name.contains("terminal")
        || tool_name.contains("command")
        || tool_name.contains("apply_patch")
        || tool_name == "applypatch";
    command_like && contains_env_command_field(tool_input)
}

fn contains_env_path_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
            let path_field = matches!(
                normalized.as_str(),
                "path"
                    | "paths"
                    | "filepath"
                    | "filepaths"
                    | "uri"
                    | "uris"
                    | "glob"
                    | "globpattern"
                    | "include"
                    | "includes"
                    | "exclude"
                    | "excludes"
            );
            (path_field && value_contains_env_reference(value)) || contains_env_path_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_env_path_field),
        _ => false,
    }
}

fn contains_env_command_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
            let command_field = matches!(
                normalized.as_str(),
                "command" | "cmd" | "script" | "patch" | "patchtext"
            );
            (command_field && value_contains_env_reference(value))
                || contains_env_command_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_env_command_field),
        _ => false,
    }
}

fn value_contains_env_reference(value: &Value) -> bool {
    match value {
        Value::String(text) => contains_env_reference(text),
        Value::Array(values) => values.iter().any(value_contains_env_reference),
        Value::Object(fields) => fields.values().any(value_contains_env_reference),
        _ => false,
    }
}

fn contains_env_reference(text: &str) -> bool {
    text.match_indices(".env").any(|(index, _)| {
        let previous = text[..index].chars().next_back();
        let next = text[index + 4..].chars().next();
        is_env_boundary_before(previous) && is_env_boundary_after(next)
    })
}

fn is_env_boundary_before(character: Option<char>) -> bool {
    character.is_none_or(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '/' | '\\' | '\'' | '"' | '`' | '=' | ':' | '(' | '[' | '{'
            )
    })
}

fn is_env_boundary_after(character: Option<char>) -> bool {
    character.is_none_or(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | '/' | '\\' | '\'' | '"' | '`' | ':' | ')' | ']' | '}' | ','
            )
    })
}

#[derive(Deserialize)]
struct RegistryData {
    #[serde(default)]
    projects: Vec<RegistryProject>,
}

#[derive(Deserialize)]
struct RegistryProject {
    root: PathBuf,
}

fn load_registered_roots() -> Result<Vec<PathBuf>, EnvError> {
    let path = if let Some(path) = std::env::var_os("ENV_MANAGER_REGISTRY_PATH") {
        PathBuf::from(path)
    } else {
        let base = directories::BaseDirs::new()
            .ok_or_else(|| EnvError::invalid("앱 데이터 경로를 확인하지 못했습니다."))?;
        base.data_dir()
            .join("dev.hgc.env-manager")
            .join("projects.json")
    };
    let bytes = fs::read(&path).map_err(|error| EnvError::io(&path, error))?;
    let registry =
        serde_json::from_slice::<RegistryData>(&bytes).map_err(EnvError::serialization)?;
    Ok(registry
        .projects
        .into_iter()
        .map(|project| project.root)
        .collect())
}

fn plan_expired() -> EnvError {
    EnvError::new(EnvErrorCode::PlanExpired, "계획이 없거나 만료되었습니다.")
}

fn audit(
    project_id: &str,
    operation: &str,
    relative_paths: &[String],
    variable_names: &[String],
    policy_decision: &str,
    result_code: &str,
) {
    let event = AuditEvent {
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
        project_id,
        actor: std::env::var("ENV_MANAGER_AGENT_HOST")
            .ok()
            .filter(|actor| matches!(actor.as_str(), "codex" | "claude-code" | "github-copilot"))
            .unwrap_or_else(|| "unknown-agent".to_owned()),
        category: audit_category(operation, policy_decision),
        operation,
        relative_paths,
        variable_names,
        policy_decision,
        outcome: if result_code == "OK" {
            "allowed"
        } else if result_code == "CODEX_ACCESS_BLOCKED" {
            "blocked"
        } else {
            "failed"
        },
        result_code,
    };
    let directory = std::env::var_os("ENV_MANAGER_AUDIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("env-manager-audit"));
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join(format!("{project_id}.jsonl"));
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 2 * 1024 * 1024) {
        let previous = directory.join(format!("{project_id}.previous.jsonl"));
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(&path, previous);
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    if serde_json::to_writer(&mut file, &event).is_ok() {
        let _ = file.write_all(b"\n");
    }
}

fn audit_category(operation: &str, policy_decision: &str) -> &'static str {
    if operation == "inspect_project" {
        "structure-inspection"
    } else if operation == "read_allowed_value" {
        "value-read"
    } else if policy_decision == "policy-change" || policy_decision == "protection-downgrade" {
        "policy-change"
    } else {
        "mutation"
    }
}

pub fn tool_definitions() -> Value {
    json!([
        tool(
            "inspect_project",
            "Return redacted env structure and value presence for a registered project.",
            json!({
                "type": "object", "properties": { "projectPath": { "type": "string" } }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "read_allowed_value",
            "Explicitly read one value only when its policy is read-write.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "key": { "type": "string" }
                }, "required": ["projectPath", "file", "key"], "additionalProperties": false
            })
        ),
        tool(
            "plan_set_allowed_value",
            "Create a redacted plan to replace a read-write value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "key": { "type": "string" }, "newValue": { "type": "string" }
                }, "required": ["projectPath", "file", "key", "newValue"], "additionalProperties": false
            })
        ),
        tool(
            "plan_create_env_file",
            "Plan creating one empty env file inside an existing registered-project directory. Existing files and example variants are rejected.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "file"], "additionalProperties": false
            })
        ),
        tool(
            "plan_add_variable",
            "Plan adding a variable with an empty value. This tool never accepts or returns a value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" }, "group": { "type": "string" },
                    "description": { "type": "array", "items": { "type": "string" } }
                }, "required": ["projectPath", "file", "key", "group"], "additionalProperties": false
            })
        ),
        tool(
            "plan_create_group",
            "Plan adding one explicit # @group marker without reading or changing values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }, "name": { "type": "string" }
                }, "required": ["projectPath", "file", "name"], "additionalProperties": false
            })
        ),
        tool(
            "plan_rename_group",
            "Plan renaming one unambiguous explicit group marker.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "currentName": { "type": "string" }, "newName": { "type": "string" }
                }, "required": ["projectPath", "file", "currentName", "newName"], "additionalProperties": false
            })
        ),
        tool(
            "plan_move_variable",
            "Plan moving an existing variable and its attached description to an existing group.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" }, "targetGroup": { "type": "string" }
                }, "required": ["projectPath", "file", "key", "targetGroup"], "additionalProperties": false
            })
        ),
        tool(
            "plan_update_description",
            "Plan replacing the ordinary comment lines attached to one variable without reading its value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" },
                    "key": { "type": "string" },
                    "lines": { "type": "array", "items": { "type": "string" } }
                }, "required": ["projectPath", "file", "key", "lines"], "additionalProperties": false
            })
        ),
        tool(
            "plan_link",
            "Plan an N-way peer link without returning any values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" },
                    "files": { "type": "array", "items": { "type": "string" }, "minItems": 2 },
                    "sourceFile": { "type": ["string", "null"] }
                }, "required": ["projectPath", "key", "files"], "additionalProperties": false
            })
        ),
        tool(
            "plan_detach",
            "Plan detaching one occurrence while preserving its current value.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "linkId": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "linkId", "file"], "additionalProperties": false
            })
        ),
        tool(
            "plan_classification",
            "Plan an explicitly requested Codex access classification without a second confirmation round trip.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" },
                    "access": { "type": "string", "enum": ["read-write", "protected", "unclassified"] }
                }, "required": ["projectPath", "key", "access"], "additionalProperties": false
            })
        ),
        tool(
            "plan_migration",
            "Plan conversion of strong visual group comments to # @group without values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "file": { "type": "string" }
                }, "required": ["projectPath", "file"], "additionalProperties": false
            })
        ),
        tool(
            "apply_plan",
            "Apply one unexpired redacted plan authorized by the current user request.",
            json!({
                "type": "object", "properties": {
                    "planId": { "type": "string" }
                }, "required": ["planId"], "additionalProperties": false
            })
        )
    ])
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

#[cfg(test)]
mod tests {
    use env_test_support::SyntheticProject;

    use super::*;

    const CANARY: &str = "fake_CANARY_never_in_projection_7f91";

    fn registered_project() -> (SyntheticProject, ProjectService) {
        let project = SyntheticProject::new();
        project.write(
            ".env.local",
            &format!("GPT_API_KEY={CANARY}\nPORT=fake_3000\n"),
        );
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        (project, service)
    }

    #[test]
    fn audit_schema_contains_only_allowlisted_metadata() {
        let paths = vec![".env.local".to_owned()];
        let keys = vec!["GPT_API_KEY".to_owned()];
        let event = AuditEvent {
            timestamp_ms: 1,
            project_id: "synthetic-project",
            actor: "claude-code".to_owned(),
            category: audit_category("read_allowed_value", "policy-checked"),
            operation: "read_allowed_value",
            relative_paths: &paths,
            variable_names: &keys,
            policy_decision: "policy-checked",
            outcome: "blocked",
            result_code: "CODEX_ACCESS_BLOCKED",
        };
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(serialized.contains("claude-code"));
        assert!(serialized.contains("GPT_API_KEY"));
        assert!(!serialized.contains(CANARY));
        for forbidden_field in ["value", "valueFragment", "replacement", "valueHash"] {
            assert!(!serialized.contains(&format!("\"{forbidden_field}\"")));
        }
    }

    #[test]
    fn guard_denies_direct_env_paths_without_echoing_input() {
        let input = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Read",
            "tool_input": {
                "file_path": "/tmp/project/.env.local",
                "content": CANARY
            }
        });

        let decision = guard_hook_decision(&input).to_string();

        assert!(decision.contains("\"permissionDecision\":\"deny\""));
        assert!(!decision.contains(CANARY));
        assert!(!decision.contains("/tmp/project"));
    }

    #[test]
    fn guard_denies_shell_and_patch_env_access() {
        for input in [
            json!({
                "tool_name": "Bash",
                "tool_input": { "command": "sed -n 1,20p apps/web/.env.development" }
            }),
            json!({
                "tool_name": "apply_patch",
                "tool_input": { "patch": "*** Update File: .env\n" }
            }),
            json!({
                "toolName": "create_file",
                "toolInput": { "filePath": "C:\\fake-project\\.env.local" }
            }),
        ] {
            assert_eq!(
                guard_hook_decision(&input)["hookSpecificOutput"]["permissionDecision"],
                "deny"
            );
        }
    }

    #[test]
    fn guard_allows_unrelated_source_operations_and_env_mentions_in_content() {
        for input in [
            json!({
                "tool_name": "Read",
                "tool_input": { "file_path": "src/main.ts" }
            }),
            json!({
                "tool_name": "Write",
                "tool_input": {
                    "file_path": "README.md",
                    "content": "Document .env.local without opening it"
                }
            }),
            json!({
                "tool_name": "Bash",
                "tool_input": { "command": "npm test" }
            }),
        ] {
            assert_eq!(guard_hook_decision(&input), json!({}));
        }
    }

    #[test]
    fn inspect_is_redacted_and_canary_free() {
        let (project, _) = registered_project();
        let output = Broker::with_registered_roots(vec![project.root().to_path_buf()])
            .call_tool(
                "inspect_project",
                json!({ "projectPath": project.root().to_string_lossy() }),
            )
            .expect("inspect")
            .to_string();
        assert!(!output.contains(CANARY));
        assert!(output.contains("GPT_API_KEY"));
        assert!(output.contains("\"valueState\":\"present\""));
    }

    #[test]
    fn protected_read_is_denied_without_leaking_canary() {
        let (project, _) = registered_project();
        let error = Broker::with_registered_roots(vec![project.root().to_path_buf()])
            .call_tool(
                "read_allowed_value",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "GPT_API_KEY"
                }),
            )
            .expect_err("protected read must fail");
        assert_eq!(error.code(), EnvErrorCode::CodexAccessBlocked);
        assert!(!error.to_string().contains(CANARY));
    }

    #[test]
    fn plan_output_never_contains_replacement_value() {
        let (project, _) = registered_project();
        let replacement = "fake_REPLACEMENT_canary_82";
        let output = Broker::with_registered_roots(vec![project.root().to_path_buf()])
            .call_tool(
                "plan_set_allowed_value",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "PORT",
                    "newValue": replacement
                }),
            )
            .expect("plan")
            .to_string();
        assert!(!output.contains(replacement));
    }

    #[test]
    fn creates_a_new_env_file_and_adds_empty_variables_without_approval() {
        let project = SyntheticProject::new();
        fs::create_dir_all(project.root().join("apps/mobile")).expect("fixture directory");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);

        let file_plan = broker
            .call_tool(
                "plan_create_env_file",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": "apps/mobile/.env"
                }),
            )
            .expect("file plan");
        let plan_id = file_plan
            .get("planId")
            .and_then(Value::as_str)
            .expect("plan id");
        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("file apply");

        for key in [
            "EXPO_PUBLIC_API_BASE_URL",
            "EXPO_PUBLIC_SUPABASE_URL",
            "EXPO_PUBLIC_SUPABASE_PUBLISHABLE_KEY",
        ] {
            let variable_plan = broker
                .call_tool(
                    "plan_add_variable",
                    json!({
                        "projectPath": project.root().to_string_lossy(),
                        "file": "apps/mobile/.env",
                        "key": key,
                        "group": "Mobile"
                    }),
                )
                .expect("variable plan");
            let plan_id = variable_plan
                .get("planId")
                .and_then(Value::as_str)
                .expect("plan id");
            broker
                .call_tool("apply_plan", json!({ "planId": plan_id }))
                .expect("variable apply");
        }

        let output = String::from_utf8(project.read("apps/mobile/.env")).expect("utf8");
        assert_eq!(output.matches("# @group Mobile").count(), 1);
        for key in [
            "EXPO_PUBLIC_API_BASE_URL",
            "EXPO_PUBLIC_SUPABASE_URL",
            "EXPO_PUBLIC_SUPABASE_PUBLISHABLE_KEY",
        ] {
            assert!(output.contains(&format!("{key}=\n")));
        }
    }

    #[test]
    fn request_authorized_plan_updates_only_an_allowed_value() {
        let (project, _) = registered_project();
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);
        let plan = broker
            .call_tool(
                "plan_set_allowed_value",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "PORT",
                    "newValue": "fake_4200"
                }),
            )
            .expect("plan");
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");
        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("apply");
        assert_eq!(
            project.read(".env.local"),
            format!("GPT_API_KEY={CANARY}\nPORT=fake_4200\n").as_bytes()
        );
    }

    #[test]
    fn apply_plan_accepts_only_the_plan_id() {
        let (project, _) = registered_project();
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);
        let plan = broker
            .call_tool(
                "plan_set_allowed_value",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "PORT",
                    "newValue": "fake_4300"
                }),
            )
            .expect("plan");
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");

        let obsolete_argument = broker
            .call_tool(
                "apply_plan",
                json!({ "planId": plan_id, "confirmed": true }),
            )
            .expect_err("obsolete confirmation argument must be rejected");
        assert_eq!(obsolete_argument.code(), EnvErrorCode::InvalidRequest);

        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("request-authorized apply");
        assert_eq!(
            project.read(".env.local"),
            format!("GPT_API_KEY={CANARY}\nPORT=fake_4300\n").as_bytes()
        );
    }

    #[test]
    fn explicitly_requested_access_change_needs_no_second_confirmation() {
        let (project, service) = registered_project();
        assert_eq!(
            service.codex_access("GPT_API_KEY").expect("initial policy"),
            CodexAccess::Protected
        );
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);
        let plan = broker
            .call_tool(
                "plan_classification",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "key": "GPT_API_KEY",
                    "access": "read-write"
                }),
            )
            .expect("classification plan");
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");

        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("classification apply");

        let reopened = ProjectService::open(project.root()).expect("service");
        assert_eq!(
            reopened
                .codex_access("GPT_API_KEY")
                .expect("updated policy"),
            CodexAccess::ReadWrite
        );
    }

    #[test]
    fn structural_group_plans_create_move_and_rename_without_value_output() {
        let (project, _) = registered_project();
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);

        for (tool_name, arguments) in [
            (
                "plan_create_group",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "name": "Database"
                }),
            ),
            (
                "plan_move_variable",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "PORT",
                    "targetGroup": "Database"
                }),
            ),
            (
                "plan_rename_group",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "currentName": "Database",
                    "newName": "Runtime"
                }),
            ),
        ] {
            let plan = broker.call_tool(tool_name, arguments).expect("plan");
            assert!(!plan.to_string().contains(CANARY));
            let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");
            broker
                .call_tool("apply_plan", json!({ "planId": plan_id }))
                .expect("apply");
        }

        let output = String::from_utf8(project.read(".env.local")).expect("utf8");
        assert!(output.contains("# @group Runtime"));
        assert!(
            output.find("# @group Runtime").expect("group") < output.find("PORT=").expect("key")
        );
        assert!(output.contains(&format!("GPT_API_KEY={CANARY}")));
    }

    #[test]
    fn codex_adds_only_an_empty_variable_and_can_update_its_description() {
        let (project, _) = registered_project();
        let broker = Broker::with_registered_roots(vec![project.root().to_path_buf()]);
        let plan = broker
            .call_tool(
                "plan_add_variable",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "DATABASE_URL",
                    "group": "Database",
                    "description": ["fake database description"]
                }),
            )
            .expect("add plan");
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");
        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("add apply");

        let added = String::from_utf8(project.read(".env.local")).expect("utf8");
        assert!(added.contains("DATABASE_URL=\n"));
        assert!(added.contains("# @group Database"));

        let description_plan = broker
            .call_tool(
                "plan_update_description",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "DATABASE_URL",
                    "lines": ["fake updated description"]
                }),
            )
            .expect("description plan");
        let plan_id = description_plan
            .get("planId")
            .and_then(Value::as_str)
            .expect("plan id");
        broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("description apply");

        let output = String::from_utf8(project.read(".env.local")).expect("utf8");
        assert!(output.contains("# fake updated description\nDATABASE_URL=\n"));
        assert!(output.contains(&format!("GPT_API_KEY={CANARY}")));
    }

    #[test]
    fn add_variable_tool_rejects_a_value_argument() {
        let (project, _) = registered_project();
        let error = Broker::with_registered_roots(vec![project.root().to_path_buf()])
            .call_tool(
                "plan_add_variable",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "file": ".env.local",
                    "key": "DATABASE_URL",
                    "group": "Database",
                    "value": "fake_must_not_be_accepted"
                }),
            )
            .expect_err("value argument must be rejected");
        assert_eq!(error.code(), EnvErrorCode::InvalidRequest);
        assert!(
            !project
                .read(".env.local")
                .windows(25)
                .any(|bytes| bytes == b"fake_must_not_be_accepted")
        );
    }

    #[test]
    fn manifest_without_active_registration_is_rejected() {
        let (project, _) = registered_project();
        let error = Broker::with_registered_roots(Vec::new())
            .call_tool(
                "inspect_project",
                json!({ "projectPath": project.root().to_string_lossy() }),
            )
            .expect_err("must reject");
        assert_eq!(error.code(), EnvErrorCode::UnregisteredProject);
    }
}

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use env_core::{
    AddVariableRequest, ClassificationSource, CodexAccess, CreateGroupRequest, EnvError,
    EnvErrorCode, LinkRequest, MigrationPlan, MoveVariableRequest, ProjectService,
    RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
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
}

enum PlannedOperation {
    SetAllowedValue(SaveValueRequest),
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
#[serde(rename_all = "camelCase")]
struct PlanClassificationArgs {
    project_path: String,
    key: String,
    access: CodexAccess,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanMigrationArgs {
    project_path: String,
    file: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyArgs {
    plan_id: String,
    confirmed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditEvent<'a> {
    timestamp_ms: u128,
    project_id: &'a str,
    actor: &'static str,
    operation: &'a str,
    relative_paths: &'a [String],
    variable_names: &'a [String],
    policy_decision: &'a str,
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
            return Err(EnvError::access_blocked(&args.key));
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
        let current = service.codex_access(&args.key)?;
        if args.access == CodexAccess::ReadWrite
            && current != CodexAccess::ReadWrite
            && !args.confirmed
        {
            return Err(EnvError::confirmation_required(&args.key));
        }
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
        if !args.confirmed {
            return Err(EnvError::invalid("계획 적용에는 명시적 확인이 필요합니다."));
        }
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
        let result = match stored.operation {
            PlannedOperation::SetAllowedValue(request) => {
                if service.codex_access(&request.key)? != CodexAccess::ReadWrite {
                    return Err(EnvError::access_blocked(&request.key));
                }
                serde_json::to_value(service.save_value(request)?)
            }
            PlannedOperation::AddVariable(request) => {
                serde_json::to_value(service.add_variable(request)?)
            }
            PlannedOperation::CreateGroup(request) => {
                serde_json::to_value(service.create_group(request)?)
            }
            PlannedOperation::RenameGroup(request) => {
                serde_json::to_value(service.rename_group(request)?)
            }
            PlannedOperation::MoveVariable(request) => {
                serde_json::to_value(service.move_variable(request)?)
            }
            PlannedOperation::UpdateDescription(request) => {
                serde_json::to_value(service.save_description(request)?)
            }
            PlannedOperation::Link(request) => serde_json::to_value(service.create_link(request)?),
            PlannedOperation::Detach { link_id, file } => {
                service.detach_link_member(&link_id, &file)?;
                Ok(json!({ "affectedFiles": [file], "keys": [] }))
            }
            PlannedOperation::Classification { key, access } => {
                service.set_codex_access_by(&key, access, ClassificationSource::Codex)?;
                Ok(json!({ "affectedFiles": [], "keys": [key] }))
            }
            PlannedOperation::Migration(plan) => {
                serde_json::to_value(service.apply_migration(plan)?)
            }
        }
        .map_err(EnvError::serialization)?;
        audit(
            service.project_id(),
            "apply_plan",
            &[],
            &[],
            "approved",
            "OK",
        );
        Ok(result)
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
            .map_or(0, |duration| duration.as_millis()),
        project_id,
        actor: "codex",
        operation,
        relative_paths,
        variable_names,
        policy_decision,
        result_code,
    };
    let directory = std::env::var_os("ENV_MANAGER_AUDIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("env-manager-audit"));
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join(format!("{project_id}.jsonl"));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    if serde_json::to_writer(&mut file, &event).is_ok() {
        let _ = file.write_all(b"\n");
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
            "Plan a Codex access classification. Downgrades to read-write require confirmed=true.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" },
                    "access": { "type": "string", "enum": ["read-write", "protected", "unclassified"] },
                    "confirmed": { "type": "boolean" }
                }, "required": ["projectPath", "key", "access", "confirmed"], "additionalProperties": false
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
            "Apply one unexpired redacted plan after user approval.",
            json!({
                "type": "object", "properties": {
                    "planId": { "type": "string" }, "confirmed": { "type": "boolean" }
                }, "required": ["planId", "confirmed"], "additionalProperties": false
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
    fn approved_plan_updates_only_an_allowed_value() {
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
            .call_tool(
                "apply_plan",
                json!({ "planId": plan_id, "confirmed": true }),
            )
            .expect("apply");
        assert_eq!(
            project.read(".env.local"),
            format!("GPT_API_KEY={CANARY}\nPORT=fake_4200\n").as_bytes()
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
                .call_tool(
                    "apply_plan",
                    json!({ "planId": plan_id, "confirmed": true }),
                )
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
            .call_tool(
                "apply_plan",
                json!({ "planId": plan_id, "confirmed": true }),
            )
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
            .call_tool(
                "apply_plan",
                json!({ "planId": plan_id, "confirmed": true }),
            )
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

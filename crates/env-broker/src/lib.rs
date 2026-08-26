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
    OpaqueValueCopyRequest, ProjectService, RedactedValueState, RenameGroupRequest,
    SaveDescriptionRequest, SaveValueRequest,
};
use env_provider::provider_push::{ProviderCompareRequest, ProviderPushRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PLAN_TTL: Duration = Duration::from_secs(300);

pub struct Broker {
    plans: Mutex<HashMap<String, StoredPlan>>,
    next_plan_id: AtomicU64,
    registered_roots_override: Option<Vec<PathBuf>>,
    provider_app_data_override: Option<PathBuf>,
    agent_host: Mutex<Option<&'static str>>,
    #[cfg(test)]
    _test_app_data: Option<tempfile::TempDir>,
}

impl Default for Broker {
    fn default() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
            registered_roots_override: None,
            provider_app_data_override: None,
            agent_host: Mutex::new(
                std::env::var("ENV_MANAGER_AGENT_HOST")
                    .ok()
                    .as_deref()
                    .and_then(normalize_agent_host),
            ),
            #[cfg(test)]
            _test_app_data: None,
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
    Detach {
        link_id: String,
        file: String,
    },
    Classification {
        key: String,
        access: CodexAccess,
    },
    Migration(MigrationPlan),
    OpaqueProjectCopy {
        source_root: PathBuf,
        source_project_id: String,
        request: OpaqueValueCopyRequest,
    },
    ProviderPush(ProviderPushRequest),
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FindReusableVariableArgs {
    project_path: String,
    key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanOpaqueProjectCopyArgs {
    project_path: String,
    source_project_id: String,
    source_file: String,
    target_file: String,
    key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReusableVariableCandidate {
    project_id: String,
    project_name: String,
    display_path: String,
    files: Vec<String>,
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
struct ListProvidersArgs {
    project_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListTeamChannelsArgs {
    project_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrokerTeamChannelProjection {
    id: String,
    name: String,
    readable: bool,
    publishable: Option<bool>,
    packages: Vec<env_team::TeamChannelPackage>,
    requires_human_passphrase: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanProviderPushArgs {
    project_path: String,
    provider: String,
    file: String,
    selections: Vec<env_provider::provider_push::ProviderSelection>,
    repository: Option<String>,
    github_environment: Option<String>,
    worker: Option<String>,
    cloudflare_environment: Option<String>,
    eas_project: Option<String>,
    #[serde(default)]
    eas_environments: Vec<String>,
    personal_target: Option<String>,
    aws_profile: Option<String>,
    aws_region: Option<String>,
    aws_path_prefix: Option<String>,
    aws_kms_key_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompareDeploymentValuesArgs {
    project_path: String,
    provider: String,
    file: String,
    keys: Vec<String>,
    aws_profile: Option<String>,
    aws_region: Option<String>,
    aws_path_prefix: Option<String>,
    runtime_target_id: Option<String>,
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
    #[cfg(test)]
    pub fn with_registered_roots(roots: Vec<PathBuf>) -> Self {
        let test_app_data = tempfile::tempdir().expect("broker test app data");
        let provider_app_data_override = Some(test_app_data.path().to_path_buf());
        Self {
            registered_roots_override: Some(roots),
            provider_app_data_override,
            _test_app_data: Some(test_app_data),
            ..Self::default()
        }
    }

    #[cfg(test)]
    fn with_registered_roots_and_app_data(roots: Vec<PathBuf>, app_data: PathBuf) -> Self {
        Self {
            registered_roots_override: Some(roots),
            provider_app_data_override: Some(app_data),
            ..Self::default()
        }
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, EnvError> {
        match name {
            "inspect_project" => self.inspect(parse(arguments)?),
            "find_reusable_variable_sources" => {
                self.find_reusable_variable_sources(parse(arguments)?)
            }
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
            "plan_copy_variable_from_project" => {
                self.plan_copy_variable_from_project(parse(arguments)?)
            }
            "list_deployment_providers" => self.list_deployment_providers(parse(arguments)?),
            "list_runtime_targets" => self.list_runtime_targets(parse(arguments)?),
            "list_team_channels" => self.list_team_channels(parse(arguments)?),
            "compare_deployment_values" => self.compare_deployment_values(parse(arguments)?),
            "plan_provider_push" => self.plan_provider_push(parse(arguments)?),
            "apply_plan" => self.apply(parse(arguments)?),
            _ => Err(EnvError::invalid("지원하지 않는 Env Manager 도구입니다.")),
        }
    }

    /// Records the MCP client identity when a host-specific environment override was
    /// not supplied. Only known host names are accepted into audit metadata.
    pub fn identify_client(&self, client_name: &str) {
        let Some(agent_host) = normalize_agent_host(client_name) else {
            return;
        };
        let mut current = self
            .agent_host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if current.is_none() {
            *current = Some(agent_host);
        }
    }

    fn inspect(&self, args: InspectArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let projection = service.scan()?;
        let result = serde_json::to_value(projection).map_err(EnvError::serialization)?;
        self.audit(
            service.project_id(),
            "inspect_project",
            &[],
            &[],
            "redacted",
            "OK",
        );
        Ok(result)
    }

    fn find_reusable_variable_sources(
        &self,
        args: FindReusableVariableArgs,
    ) -> Result<Value, EnvError> {
        let target = self.open_registered(&args.project_path)?;
        let mut candidates = Vec::new();
        for registration in self.registered_projects()? {
            let Ok(service) = ProjectService::open(&registration.root) else {
                continue;
            };
            if service.project_id() == target.project_id()
                || !service.root().join(env_core::MANIFEST_FILE_NAME).is_file()
            {
                continue;
            }
            let Ok(occurrences) = service.redacted_occurrences(&args.key) else {
                continue;
            };
            let files = occurrences
                .into_iter()
                .filter(|occurrence| occurrence.value_state == RedactedValueState::Present)
                .map(|occurrence| occurrence.file)
                .collect::<Vec<_>>();
            if files.is_empty() {
                continue;
            }
            candidates.push(ReusableVariableCandidate {
                project_id: service.project_id().to_owned(),
                project_name: registration.name,
                display_path: registration.display_path,
                files,
            });
        }
        candidates.sort_by(|left, right| {
            left.project_name
                .to_ascii_lowercase()
                .cmp(&right.project_name.to_ascii_lowercase())
                .then_with(|| left.project_id.cmp(&right.project_id))
        });
        self.audit(
            target.project_id(),
            "find_reusable_variable_sources",
            &[],
            std::slice::from_ref(&args.key),
            "redacted-cross-project-search",
            "OK",
        );
        Ok(json!({ "candidates": candidates }))
    }

    fn read_allowed(&self, args: ValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let value = service.read_allowed_value(&args.file, &args.key);
        let code = value
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        self.audit(
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
            self.audit(
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

    fn plan_copy_variable_from_project(
        &self,
        args: PlanOpaqueProjectCopyArgs,
    ) -> Result<Value, EnvError> {
        let target = self.open_registered(&args.project_path)?;
        let source = self.open_registered_project_id(&args.source_project_id)?;
        if source.project_id() == target.project_id() {
            return Err(EnvError::invalid(
                "같은 프로젝트 안에서는 기존 연결 또는 값 편집 기능을 사용해주세요.",
            ));
        }
        let source_available =
            source
                .redacted_occurrences(&args.key)?
                .into_iter()
                .any(|occurrence| {
                    occurrence.file == args.source_file
                        && occurrence.value_state == RedactedValueState::Present
                });
        if !source_available {
            return Err(EnvError::invalid(format!(
                "선택한 원본에 값이 있는 {} 변수를 찾지 못했습니다.",
                args.key
            )));
        }
        let affected_files = target.opaque_copy_impact(&args.target_file, &args.key)?;
        let request = OpaqueValueCopyRequest {
            source_file: args.source_file,
            target_file: args.target_file,
            key: args.key.clone(),
        };
        self.store_plan(
            &target,
            PlannedOperation::OpaqueProjectCopy {
                source_root: source.root().to_path_buf(),
                source_project_id: source.project_id().to_owned(),
                request,
            },
            format!(
                "다른 등록 프로젝트의 {} 값을 실제 값 노출 없이 현재 프로젝트로 한 번 복사합니다.",
                args.key
            ),
            affected_files,
            vec![args.key],
            "cross-project-value-copy",
            None,
        )
    }

    fn list_deployment_providers(&self, args: ListProvidersArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let app_data = self.provider_app_data()?;
        let providers = env_provider::provider_push::list(service.root(), &app_data);
        self.audit(
            service.project_id(),
            "list_deployment_providers",
            &[],
            &[],
            "redacted-provider-metadata",
            "OK",
        );
        serde_json::to_value(providers).map_err(EnvError::serialization)
    }

    fn list_runtime_targets(&self, args: ListProvidersArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let targets = env_provider::runtime_target::list(service.root())
            .map_err(provider_error)?
            .into_iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "displayName": target.display_name,
                    "sourceFile": target.source_file,
                    "transport": target.transport_label(),
                })
            })
            .collect::<Vec<_>>();
        self.audit(
            service.project_id(),
            "list_runtime_targets",
            &[],
            &[],
            "redacted-runtime-target-metadata",
            "OK",
        );
        Ok(Value::Array(targets))
    }

    fn list_team_channels(&self, args: ListTeamChannelsArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let registry = load_registry_data(&self.provider_app_data()?.join("projects.json"))?;
        let channels = registry
            .team_channels
            .into_iter()
            .filter(|channel| channel.project_id == service.project_id())
            .map(|channel| {
                let transport = match env_team::open_transport(&channel.transport) {
                    Ok(transport) => transport,
                    Err(error) if error.code() == EnvErrorCode::Io => {
                        return Ok(BrokerTeamChannelProjection {
                            id: channel.id,
                            name: channel.name,
                            readable: false,
                            publishable: None,
                            packages: Vec::new(),
                            requires_human_passphrase: true,
                        });
                    }
                    Err(error) => return Err(error),
                };
                let capabilities = transport.inspect(env_team::CapabilityProbe::ReadOnly)?;
                let packages = if capabilities.readable {
                    transport.list_packages()?
                } else {
                    Vec::new()
                };
                Ok(BrokerTeamChannelProjection {
                    id: channel.id,
                    name: channel.name,
                    readable: capabilities.readable,
                    publishable: capabilities.publishable,
                    packages,
                    requires_human_passphrase: true,
                })
            })
            .collect::<Result<Vec<_>, EnvError>>()?;
        self.audit(
            service.project_id(),
            "list_team_channels",
            &[],
            &[],
            "redacted-channel-metadata",
            "OK",
        );
        serde_json::to_value(channels).map_err(EnvError::serialization)
    }

    fn plan_provider_push(&self, args: PlanProviderPushArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        if args.selections.is_empty() || args.selections.len() > 100 {
            return Err(EnvError::invalid("전송할 변수를 1개 이상 선택해주세요."));
        }
        let keys = args
            .selections
            .iter()
            .map(|selection| selection.key.clone())
            .collect::<Vec<_>>();
        let unique = keys.iter().collect::<std::collections::BTreeSet<_>>();
        if unique.len() != keys.len() {
            return Err(EnvError::invalid("같은 변수를 중복 선택할 수 없습니다."));
        }
        let destination = if args.provider == "expo-eas" {
            match args.eas_project.as_deref() {
                Some(project) => format!("{project} [{}]", args.eas_environments.join(", ")),
                None => "대상 미지정".to_owned(),
            }
        } else {
            args.provider.clone()
        };
        let request = ProviderPushRequest {
            provider: args.provider.clone(),
            file: args.file.clone(),
            selections: args.selections,
            repository: args.repository,
            github_environment: args.github_environment,
            worker: args.worker,
            cloudflare_environment: args.cloudflare_environment,
            eas_project: args.eas_project,
            eas_environments: args.eas_environments,
            personal_target: args.personal_target,
            aws_profile: args.aws_profile,
            aws_region: args.aws_region,
            aws_path_prefix: args.aws_path_prefix,
            aws_kms_key_id: args.aws_kms_key_id,
        };
        self.store_plan(
            &service,
            PlannedOperation::ProviderPush(request),
            format!(
                "{}의 환경변수 {}개를 {} 대상으로 값 노출 없이 전송합니다.",
                args.file,
                keys.len(),
                destination
            ),
            vec![args.file],
            keys,
            "opaque-provider-push",
            None,
        )
    }

    fn compare_deployment_values(
        &self,
        args: CompareDeploymentValuesArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let file = args.file.clone();
        let keys = args.keys.clone();
        let comparison = env_provider::provider_push::compare(
            &service,
            ProviderCompareRequest {
                provider: args.provider,
                file: args.file,
                keys: args.keys,
                aws_profile: args.aws_profile,
                aws_region: args.aws_region,
                aws_path_prefix: args.aws_path_prefix,
                runtime_target_id: args.runtime_target_id,
            },
        )
        .map_err(provider_error);
        let result_code = comparison
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        self.audit(
            service.project_id(),
            "compare_deployment_values",
            std::slice::from_ref(&file),
            &keys,
            "opaque-provider-compare",
            result_code,
        );
        let comparison = comparison?;
        serde_json::to_value(comparison).map_err(EnvError::serialization)
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
        self.audit(
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

        let service = self.open_registered_root(&stored.root)?;
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
                    self.audit(
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
            PlannedOperation::OpaqueProjectCopy {
                source_root,
                source_project_id,
                request,
            } => {
                let source = self.open_registered_root(&source_root)?;
                if source.project_id() != source_project_id {
                    return Err(EnvError::unregistered_project(&source_project_id));
                }
                let source_file = request.source_file.clone();
                let key = request.key.clone();
                let copied = service.copy_value_from(&source, request);
                let source_result_code = copied
                    .as_ref()
                    .map_or_else(|error| error.code().as_str(), |_| "OK");
                self.audit(
                    source.project_id(),
                    "copy_variable_to_registered_project",
                    std::slice::from_ref(&source_file),
                    std::slice::from_ref(&key),
                    "opaque-cross-project-source",
                    source_result_code,
                );
                serialize_result(copied)
            }
            PlannedOperation::ProviderPush(request) => {
                let app_data = self.provider_app_data()?;
                env_provider::provider_push::push(&service, &app_data, request)
                    .map_err(provider_error)
                    .and_then(|result| {
                        serde_json::to_value(result).map_err(EnvError::serialization)
                    })
            }
        };
        let result_code = result
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        self.audit(
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
        self.open_registered_root(&root)
    }

    fn open_registered_root(&self, root: &Path) -> Result<ProjectService, EnvError> {
        let registered = self.registered_projects()?.into_iter().any(|candidate| {
            candidate
                .root
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

    fn open_registered_project_id(&self, project_id: &str) -> Result<ProjectService, EnvError> {
        for registration in self.registered_projects()? {
            let Ok(service) = ProjectService::open(&registration.root) else {
                continue;
            };
            if service.project_id() == project_id
                && service.root().join(env_core::MANIFEST_FILE_NAME).is_file()
            {
                return Ok(service);
            }
        }
        Err(EnvError::unregistered_project(project_id))
    }

    fn registered_projects(&self) -> Result<Vec<RegisteredProject>, EnvError> {
        if let Some(roots) = &self.registered_roots_override {
            return Ok(roots
                .iter()
                .map(|root| RegisteredProject {
                    name: root.file_name().map_or_else(
                        || "Project".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
                    display_path: root.to_string_lossy().into_owned(),
                    root: root.clone(),
                })
                .collect());
        }
        load_registered_projects()
    }

    fn provider_app_data(&self) -> Result<PathBuf, EnvError> {
        if let Some(path) = &self.provider_app_data_override {
            return Ok(path.clone());
        }
        provider_app_data()
    }

    fn audit(
        &self,
        project_id: &str,
        operation: &str,
        relative_paths: &[String],
        variable_names: &[String],
        policy_decision: &str,
        result_code: &str,
    ) {
        let actor = self
            .agent_host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unwrap_or("unknown-agent")
            .to_owned();
        let event = AuditEvent {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis() as u64),
            project_id,
            actor,
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
            .or_else(|| {
                self.provider_app_data()
                    .ok()
                    .map(|path| path.join("agent-activity"))
            })
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
#[serde(rename_all = "camelCase")]
struct RegistryData {
    #[serde(default)]
    projects: Vec<RegistryProject>,
    #[serde(default)]
    #[serde(deserialize_with = "env_team::deserialize_team_channel_registrations")]
    team_channels: Vec<env_team::TeamChannelRegistration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryProject {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    display_path: Option<String>,
    root: PathBuf,
}

struct RegisteredProject {
    name: String,
    display_path: String,
    root: PathBuf,
}

fn load_registered_projects() -> Result<Vec<RegisteredProject>, EnvError> {
    let path = if let Some(path) = std::env::var_os("ENV_MANAGER_REGISTRY_PATH") {
        PathBuf::from(path)
    } else {
        let base = directories::BaseDirs::new()
            .ok_or_else(|| EnvError::invalid("앱 데이터 경로를 확인하지 못했습니다."))?;
        base.data_dir()
            .join("dev.hgc.env-manager")
            .join("projects.json")
    };
    let registry = load_registry_data(&path)?;
    Ok(registry
        .projects
        .into_iter()
        .map(|project| RegisteredProject {
            name: project.name.unwrap_or_else(|| {
                project.root.file_name().map_or_else(
                    || "Project".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                )
            }),
            display_path: project
                .display_path
                .unwrap_or_else(|| project.root.to_string_lossy().into_owned()),
            root: project.root,
        })
        .collect())
}

fn load_registry_data(path: &Path) -> Result<RegistryData, EnvError> {
    let bytes = fs::read(path).map_err(|error| EnvError::io(path, error))?;
    serde_json::from_slice::<RegistryData>(&bytes).map_err(EnvError::serialization)
}

fn plan_expired() -> EnvError {
    EnvError::new(EnvErrorCode::PlanExpired, "계획이 없거나 만료되었습니다.")
}

fn provider_app_data() -> Result<PathBuf, EnvError> {
    if let Some(path) = std::env::var_os("ENV_MANAGER_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    let base = directories::BaseDirs::new()
        .ok_or_else(|| EnvError::invalid("앱 데이터 경로를 확인하지 못했습니다."))?;
    Ok(base.data_dir().join("dev.hgc.env-manager"))
}

fn provider_error(error: env_provider::provider_push::ProviderPushError) -> EnvError {
    EnvError::invalid(format!("{}: {}", error.code, error.message))
}

fn audit_category(operation: &str, policy_decision: &str) -> &'static str {
    if matches!(
        operation,
        "inspect_project" | "list_team_channels" | "list_runtime_targets"
    ) {
        "structure-inspection"
    } else if operation == "read_allowed_value" {
        "value-read"
    } else if operation == "compare_deployment_values" {
        "provider-compare"
    } else if policy_decision == "policy-change" || policy_decision == "protection-downgrade" {
        "policy-change"
    } else {
        "mutation"
    }
}

fn normalize_agent_host(client_name: &str) -> Option<&'static str> {
    let normalized = client_name.to_ascii_lowercase();
    if normalized.contains("codex") {
        Some("codex")
    } else if normalized.contains("claude") {
        Some("claude-code")
    } else if normalized.contains("copilot") {
        Some("github-copilot")
    } else {
        None
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
            "find_reusable_variable_sources",
            "Find same-name variables with present values in other registered projects. Returns project and file metadata only, never values.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }, "key": { "type": "string" }
                }, "required": ["projectPath", "key"], "additionalProperties": false
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
            "plan_copy_variable_from_project",
            "Plan a one-time opaque copy of one same-name value from another registered project. The value is handled only inside Rust and is never returned to the agent.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" },
                    "sourceProjectId": { "type": "string" },
                    "sourceFile": { "type": "string" },
                    "targetFile": { "type": "string" },
                    "key": { "type": "string" }
                },
                "required": ["projectPath", "sourceProjectId", "sourceFile", "targetFile", "key"],
                "additionalProperties": false
            })
        ),
        tool(
            "list_deployment_providers",
            "List official and locally installed providers with availability and version metadata. Never returns values or commands.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "list_runtime_targets",
            "List registered fixed-verifier Runtime targets for a project. Returns target IDs, display names, source files, and transport labels only; never returns recipients, destinations, remote paths, values, or commands.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "list_team_channels",
            "List connected Folder Team Channels and encrypted-package metadata for a registered project. Never returns folder paths, values, passphrases, or decrypted content. Passphrase publish/import remains a desktop action.",
            json!({
                "type": "object", "properties": {
                    "projectPath": { "type": "string" }
                }, "required": ["projectPath"], "additionalProperties": false
            })
        ),
        tool(
            "compare_deployment_values",
            "Compare selected managed values with a supported deployment target. Returns equality states only; never accepts or returns candidate values, hashes, or provider output.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": { "type": "string" },
                    "provider": { "type": "string" },
                    "file": { "type": "string" },
                    "keys": {
                        "type": "array", "minItems": 1, "maxItems": 100,
                        "items": { "type": "string" }
                    },
                    "awsProfile": { "type": ["string", "null"] },
                    "awsRegion": { "type": ["string", "null"] },
                    "awsPathPrefix": { "type": ["string", "null"] }
                    ,"runtimeTargetId": { "type": ["string", "null"] }
                },
                "required": ["projectPath", "provider", "file", "keys"],
                "additionalProperties": false
            })
        ),
        tool(
            "plan_provider_push",
            "Create a redacted one-way provider push plan. Values remain inside Rust and are resolved only when apply_plan is called.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": { "type": "string" },
                    "provider": { "type": "string" },
                    "file": { "type": "string" },
                    "selections": {
                        "type": "array", "minItems": 1, "maxItems": 100,
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "kind": { "type": "string", "enum": ["secret", "variable", "plaintext", "sensitive"] }
                            },
                            "required": ["key", "kind"],
                            "additionalProperties": false
                        }
                    },
                    "repository": { "type": ["string", "null"] },
                    "githubEnvironment": { "type": ["string", "null"] },
                    "worker": { "type": ["string", "null"] },
                    "cloudflareEnvironment": { "type": ["string", "null"] },
                    "easProject": { "type": ["string", "null"] },
                    "easEnvironments": {
                        "type": "array", "maxItems": 10,
                        "items": { "type": "string" }
                    },
                    "personalTarget": { "type": ["string", "null"] }
                    ,"awsProfile": { "type": ["string", "null"] }
                    ,"awsRegion": { "type": ["string", "null"] }
                    ,"awsPathPrefix": { "type": ["string", "null"] }
                    ,"awsKmsKeyId": { "type": ["string", "null"] }
                },
                "required": ["projectPath", "provider", "file", "selections"],
                "additionalProperties": false
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
        service
            .set_codex_access("PORT", CodexAccess::ReadWrite)
            .expect("explicitly allow synthetic runtime setting");
        (project, service)
    }

    #[test]
    fn team_channel_listing_returns_ciphertext_metadata_only() {
        let (project, service) = registered_project();
        let app_data = tempfile::tempdir().expect("app data");
        let shared = tempfile::tempdir().expect("shared folder");
        let channel = env_team::connect_folder_transport(shared.path(), "Synthetic team")
            .expect("connect synthetic channel");
        let env_team::TeamChannelTransportConfig::Folder { channel_id, .. } = &channel.transport;
        let transport = env_team::open_transport(&channel.transport).expect("transport");
        let package = transport
            .publish(&mut std::io::Cursor::new(
                b"fake ciphertext without env values",
            ))
            .expect("synthetic ciphertext");
        let package_id = package.id;
        fs::write(
            app_data.path().join("projects.json"),
            serde_json::to_vec(&json!({
                "projects": [{
                    "name": "Synthetic project",
                    "displayPath": project.root().to_string_lossy(),
                    "root": project.root(),
                }],
                "teamChannels": [{
                    "id": "folder_local_12345678",
                    "projectId": service.project_id(),
                    "channelId": channel_id,
                    "name": "Synthetic team",
                    "root": shared.path(),
                }]
            }))
            .expect("registry json"),
        )
        .expect("registry");
        let broker = Broker::with_registered_roots_and_app_data(
            vec![project.root().to_path_buf()],
            app_data.path().to_path_buf(),
        );

        let result = broker
            .call_tool(
                "list_team_channels",
                json!({ "projectPath": project.root().to_string_lossy() }),
            )
            .expect("list channels");
        let output = result.to_string();
        assert!(output.contains(&package_id), "{output}");
        assert!(output.contains("requiresHumanPassphrase"));
        assert!(!output.contains(CANARY));
        assert!(!output.contains(&shared.path().to_string_lossy().to_string()));
        assert!(!output.contains("passphrase\":"));
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
    fn audit_uses_app_data_without_plugin_environment_and_identifies_mcp_client() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = SyntheticProject::new();
        project.write(".env.local", "CLIENT_MODE=fake_client_value\n");
        let service = ProjectService::open(project.root()).expect("project service");
        service.initialize().expect("initialize project");
        let project_id = service.project_id().to_owned();
        let broker = Broker::with_registered_roots_and_app_data(
            vec![project.root().to_path_buf()],
            app_data.path().to_path_buf(),
        );
        broker.identify_client("codex-mcp-client");

        broker
            .call_tool(
                "inspect_project",
                json!({ "projectPath": project.root().to_string_lossy() }),
            )
            .expect("inspect through broker");

        let audit = fs::read_to_string(
            app_data
                .path()
                .join("agent-activity")
                .join(format!("{project_id}.jsonl")),
        )
        .expect("app-owned audit log");
        assert!(audit.contains(r#""actor":"codex""#));
        assert!(audit.contains(r#""operation":"inspect_project""#));
        assert!(!audit.contains("fake_client_value"));
    }

    #[test]
    fn unknown_mcp_client_names_stay_unattributed() {
        assert_eq!(normalize_agent_host("Codex Desktop"), Some("codex"));
        assert_eq!(normalize_agent_host("claude-code"), Some("claude-code"));
        assert_eq!(
            normalize_agent_host("GitHub Copilot"),
            Some("github-copilot")
        );
        assert_eq!(normalize_agent_host("custom-agent"), None);
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
    fn finds_and_copies_a_protected_value_across_registered_projects_opaquely() {
        let source = SyntheticProject::new();
        let target = SyntheticProject::new();
        let cross_project_canary = "fake_CROSS_PROJECT_BROKER_CANARY_92";
        source.write(
            ".env.local",
            &format!("GEMINI_API_KEY={cross_project_canary}\n"),
        );
        target.write(".env.local", "GEMINI_API_KEY=\n");
        let source_service = ProjectService::open(source.root()).expect("source service");
        let target_service = ProjectService::open(target.root()).expect("target service");
        source_service.initialize().expect("source initialize");
        target_service.initialize().expect("target initialize");
        let broker = Broker::with_registered_roots(vec![
            source.root().to_path_buf(),
            target.root().to_path_buf(),
        ]);

        let candidates = broker
            .call_tool(
                "find_reusable_variable_sources",
                json!({
                    "projectPath": target.root().to_string_lossy(),
                    "key": "GEMINI_API_KEY"
                }),
            )
            .expect("candidate search");
        let candidate_output = candidates.to_string();
        assert!(candidate_output.contains(source_service.project_id()));
        assert!(candidate_output.contains(".env.local"));
        assert!(!candidate_output.contains(cross_project_canary));

        let plan = broker
            .call_tool(
                "plan_copy_variable_from_project",
                json!({
                    "projectPath": target.root().to_string_lossy(),
                    "sourceProjectId": source_service.project_id(),
                    "sourceFile": ".env.local",
                    "targetFile": ".env.local",
                    "key": "GEMINI_API_KEY"
                }),
            )
            .expect("opaque copy plan");
        assert!(!plan.to_string().contains(cross_project_canary));
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");
        let result = broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("opaque copy apply");

        assert!(!result.to_string().contains(cross_project_canary));
        assert_eq!(
            target.read(".env.local"),
            format!("GEMINI_API_KEY={cross_project_canary}\n").as_bytes()
        );
        assert_eq!(
            source_service
                .codex_access("GEMINI_API_KEY")
                .expect("source policy"),
            CodexAccess::Protected
        );
        assert_eq!(
            target_service
                .codex_access("GEMINI_API_KEY")
                .expect("target policy"),
            CodexAccess::Protected
        );
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

    #[cfg(unix)]
    #[test]
    fn personal_provider_push_keeps_the_value_out_of_agent_arguments_and_results() {
        use std::os::unix::fs::PermissionsExt;

        let (project, _) = registered_project();
        let app_data = tempfile::tempdir().expect("app data");
        let pack_source = tempfile::tempdir().expect("pack source");
        let runner_dir = tempfile::tempdir().expect("runner");
        let executable = runner_dir.path().join("fake-provider");
        let args_capture = runner_dir.path().join("args.txt");
        let stdin_capture = runner_dir.path().join("stdin.txt");
        fs::write(
            &executable,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '1.2.3\\n'; exit 0; fi\nargs_file=$1\nstdin_file=$2\nshift 2\nprintf '%s\\n' \"$@\" > \"$args_file\"\ncat > \"$stdin_file\"\n",
        )
        .expect("runner");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("permissions");
        fs::write(
            pack_source.path().join("provider.json"),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 1,
                "id": "local.test.capture",
                "displayName": "Capture Provider",
                "description": "Synthetic provider",
                "version": "1.0.0",
                "providerProtocolVersion": "0.2.0",
                "valueTransport": "stdin",
                "target": { "label": "Application", "placeholder": "target" },
                "cli": {
                    "executableCandidates": [executable.to_string_lossy()],
                    "versionArgs": ["--version"],
                    "profiles": [{
                        "id": "capture-v1",
                        "versionRequirement": ">=1.0.0,<2.0.0",
                        "pushArgs": [
                            args_capture.to_string_lossy(),
                            stdin_capture.to_string_lossy(),
                            "push", "{key}", "--app", "{target}"
                        ]
                    }]
                }
            }))
            .expect("manifest"),
        )
        .expect("write manifest");
        env_provider::personal_provider::install(pack_source.path(), app_data.path(), false)
            .expect("install pack");

        let broker = Broker::with_registered_roots_and_app_data(
            vec![project.root().to_path_buf()],
            app_data.path().to_path_buf(),
        );
        let plan = broker
            .call_tool(
                "plan_provider_push",
                json!({
                    "projectPath": project.root().to_string_lossy(),
                    "provider": "local.test.capture",
                    "file": ".env.local",
                    "selections": [{ "key": "GPT_API_KEY", "kind": "secret" }],
                    "personalTarget": "fake-app"
                }),
            )
            .expect("provider plan");
        assert!(!plan.to_string().contains(CANARY));
        let plan_id = plan.get("planId").and_then(Value::as_str).expect("plan id");
        let result = broker
            .call_tool("apply_plan", json!({ "planId": plan_id }))
            .expect("provider apply");
        assert!(!result.to_string().contains(CANARY));
        assert_eq!(fs::read_to_string(stdin_capture).expect("stdin"), CANARY);
        let arguments = fs::read_to_string(args_capture).expect("args");
        assert!(arguments.contains("GPT_API_KEY"));
        assert!(arguments.contains("fake-app"));
        assert!(!arguments.contains(CANARY));
    }

    #[test]
    fn provider_compare_returns_only_redacted_state_for_protected_values() {
        let (project, service) = registered_project();
        let broker = Broker::with_registered_roots(vec![service.root().to_path_buf()]);
        let result = broker
            .call_tool(
                "compare_deployment_values",
                json!({
                    "projectPath": project.root(),
                    "provider": "github-actions",
                    "file": ".env.local",
                    "keys": ["GPT_API_KEY"]
                }),
            )
            .expect("redacted provider comparison");

        assert_eq!(result["items"][0]["state"], "unverifiable");
        assert!(!result.to_string().contains(CANARY));
        assert_eq!(
            service
                .codex_access("GPT_API_KEY")
                .expect("protected access"),
            CodexAccess::Protected
        );
    }

    #[test]
    fn runtime_target_listing_omits_destination_recipient_and_remote_path() {
        let (project, service) = registered_project();
        let identity = age::x25519::Identity::generate();
        env_provider::runtime_target::save(
            service.root(),
            env_provider::runtime_target::RuntimeTarget {
                id: "mobile-ok-dev".to_owned(),
                display_name: "mobile-ok · dev".to_owned(),
                source_file: ".env.local".to_owned(),
                remote_target_id: "server-mobile-ok-dev".to_owned(),
                recipient: identity.to_public().to_string(),
                transport: env_provider::runtime_target::RuntimeTransport::Ssh {
                    destination: "deploy@private.example.test".to_owned(),
                },
            },
        )
        .expect("save target fixture");
        let broker = Broker::with_registered_roots(vec![service.root().to_path_buf()]);
        let result = broker
            .call_tool(
                "list_runtime_targets",
                json!({ "projectPath": project.root() }),
            )
            .expect("list runtime targets");

        assert_eq!(result[0]["id"], "mobile-ok-dev");
        assert_eq!(result[0]["sourceFile"], ".env.local");
        assert_eq!(result[0]["transport"], "SSH");
        let serialized = result.to_string();
        assert!(!serialized.contains("private.example.test"));
        assert!(!serialized.contains("age1"));
        assert!(!serialized.contains("server-mobile-ok-dev"));
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

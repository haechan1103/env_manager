use super::super::*;
use super::project_tools::plan_expired;
use super::provider_tools::{action_pack_error, provider_error};

impl Broker {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn store_plan(
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

    pub(super) fn apply(&self, args: ApplyArgs) -> Result<Value, EnvError> {
        let stored = self
            .plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&args.plan_id)
            .ok_or_else(plan_expired)?;
        if stored.expires_at < Instant::now() {
            return Err(plan_expired());
        }

        if matches!(stored.operation, PlannedOperation::RegisterProject) {
            return self.apply_project_registration(stored);
        }

        let service = self.open_registered_root(&stored.root)?;
        if service.project_id() != stored.project_id {
            return Err(EnvError::unregistered_project(&stored.project_id));
        }
        let affected_files = stored.affected_files.clone();
        let keys = stored.keys.clone();
        let risk = stored.risk;
        let result: Result<Value, EnvError> = match stored.operation {
            PlannedOperation::RegisterProject => unreachable!("registration handled above"),
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
            PlannedOperation::ActionPack(request) => {
                let app_data = self.provider_app_data()?;
                env_provider::action_pack::execute(&service, &app_data, request)
                    .map_err(action_pack_error)
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

    pub(super) fn apply_project_registration(&self, stored: StoredPlan) -> Result<Value, EnvError> {
        let service = self.open_current_workspace_candidate()?;
        if service.root() != stored.root || service.project_id() != stored.project_id {
            return Err(EnvError::invalid(
                "계획을 만든 작업 프로젝트가 변경되었습니다. 등록 계획을 다시 만들어주세요.",
            ));
        }

        // Initialization may inspect values inside privileged Rust, but its projection is
        // deliberately discarded so no value crosses the broker boundary.
        service.initialize()?;
        let name = service.root().file_name().map_or_else(
            || "Project".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let registration = ProjectRegistration {
            id: service.project_id().to_owned(),
            name: name.clone(),
            display_path: service.root().to_string_lossy().into_owned(),
            root: service.root().to_path_buf(),
            file_labels: Default::default(),
        };
        let registry_path = self.registry_path()?;
        env_registry::update(&registry_path, |registry| {
            if let Some(existing) = registry
                .projects
                .iter_mut()
                .find(|project| project.id == registration.id)
            {
                existing.display_path = registration.display_path.clone();
                existing.root = registration.root.clone();
            } else {
                registry.projects.push(registration.clone());
                registry.projects.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                });
            }
            Ok(())
        })?;
        self.audit(
            service.project_id(),
            "register_current_project",
            &[],
            &[],
            stored.risk,
            "OK",
        );
        Ok(json!({
            "projectId": service.project_id(),
            "name": name,
            "displayPath": service.root(),
            "registered": true
        }))
    }
}

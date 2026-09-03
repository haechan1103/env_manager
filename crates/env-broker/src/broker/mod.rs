mod audit;
mod env_tools;
pub(super) mod guard;
mod plan;
mod project_tools;
mod provider_tools;
pub(super) mod tool_schema;

use super::*;

pub(crate) use audit::append_audit_event;
#[cfg(test)]
pub(crate) use audit::{audit_category, normalize_agent_host};
pub(super) use project_tools::provider_app_data;

impl Default for Broker {
    fn default() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
            registered_roots_override: None,
            provider_app_data_override: None,
            workspace_root_override: None,
            agent_host: Mutex::new(
                std::env::var("ENV_MANAGER_AGENT_HOST")
                    .ok()
                    .as_deref()
                    .and_then(audit::normalize_agent_host),
            ),
            #[cfg(test)]
            _test_app_data: None,
        }
    }
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
    pub(super) fn with_registered_roots_and_app_data(
        roots: Vec<PathBuf>,
        app_data: PathBuf,
    ) -> Self {
        Self {
            registered_roots_override: Some(roots),
            provider_app_data_override: Some(app_data),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(super) fn with_workspace_and_app_data(workspace: PathBuf, app_data: PathBuf) -> Self {
        Self {
            provider_app_data_override: Some(app_data),
            workspace_root_override: Some(workspace),
            ..Self::default()
        }
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, EnvError> {
        match name {
            "plan_register_current_project" => {
                self.plan_register_current_project(parse(arguments)?)
            }
            "inspect_project" => self.inspect(parse(arguments)?),
            "find_reusable_variable_sources" => {
                self.find_reusable_variable_sources(parse(arguments)?)
            }
            "read_allowed_value" => self.read_allowed(parse(arguments)?),
            "plan_set_allowed_value" => self.plan_value(parse(arguments)?),
            "plan_stdin_value_write" => self.plan_stdin_value(parse(arguments)?),
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
            "list_action_packs" => self.list_action_packs(parse(arguments)?),
            "list_runtime_targets" => self.list_runtime_targets(parse(arguments)?),
            "list_team_channels" => self.list_team_channels(parse(arguments)?),
            "compare_deployment_values" => self.compare_deployment_values(parse(arguments)?),
            "plan_provider_push" => self.plan_provider_push(parse(arguments)?),
            "plan_action" => self.plan_action(parse(arguments)?),
            "apply_plan" => self.apply(parse(arguments)?),
            _ => Err(EnvError::invalid("지원하지 않는 Kavranta 도구입니다.")),
        }
    }

    /// Records the MCP client identity when a host-specific environment override was
    /// not supplied. Only known host names are accepted into audit metadata.
    pub fn identify_client(&self, client_name: &str) {
        let Some(agent_host) = audit::normalize_agent_host(client_name) else {
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
}

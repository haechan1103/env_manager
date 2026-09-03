use super::super::*;

impl Broker {
    pub(super) fn audit(
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
        append_audit_event(
            self.provider_app_data().ok().as_deref(),
            project_id,
            actor,
            operation,
            relative_paths,
            variable_names,
            policy_decision,
            result_code,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_audit_event(
    app_data: Option<&Path>,
    project_id: &str,
    actor: String,
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
        .or_else(|| app_data.map(|path| path.join("agent-activity")))
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

pub(crate) fn audit_category(operation: &str, policy_decision: &str) -> &'static str {
    if operation == "register_current_project" {
        "project-registration"
    } else if matches!(
        operation,
        "inspect_project" | "list_team_channels" | "list_runtime_targets" | "list_action_packs"
    ) {
        "structure-inspection"
    } else if operation == "read_allowed_value" {
        "value-read"
    } else if operation == "compare_deployment_values" {
        "provider-compare"
    } else if policy_decision == "opaque-action-pack" {
        "action-execution"
    } else if policy_decision == "policy-change" || policy_decision == "protection-downgrade" {
        "policy-change"
    } else {
        "mutation"
    }
}

pub(crate) fn normalize_agent_host(client_name: &str) -> Option<&'static str> {
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

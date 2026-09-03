mod broker;
mod catalog;
mod command;
mod installation;
mod marketplace;
mod model;

use std::path::Path;

use tauri::AppHandle;

use broker::ensure_current_broker;
use catalog::{catalog_source, materialize_catalog};
use command::{detect_vscode, integration_executable, run_agent_command};
use installation::{
    cached_bundle_is_official, connection_configuration_is_current, current_bundle_is_cached,
    installed_version, marker_version, official_codex_marketplace_aliases, persist_marker,
    remove_legacy_codex_plugin,
};
use marketplace::{
    install_or_update, marketplace_add_args, reconnect_owned_marketplace,
    refresh_after_marketplace_reconnect, refresh_owned_codex_marketplace,
};
use model::{
    agent_bundle_version, integration_name, is_legacy_bundle_version, is_update_available,
};

pub use model::{
    AgentIntegrationBlocker, AgentIntegrationId, AgentIntegrationStatus, IntegrationError,
};

/// Returns redacted integration health for every supported agent host.
pub fn list(app: &AppHandle) -> Vec<AgentIntegrationStatus> {
    let broker = ensure_current_broker(app).ok();
    let catalog_available = catalog_source(app).is_some();
    [
        AgentIntegrationId::Codex,
        AgentIntegrationId::ClaudeCode,
        AgentIntegrationId::GithubCopilot,
    ]
    .into_iter()
    .map(|id| status(app, id, broker.as_deref(), catalog_available))
    .collect()
}

/// Installs or repairs one supported agent integration from the bundled catalog.
pub fn install(
    app: &AppHandle,
    id: AgentIntegrationId,
) -> Result<AgentIntegrationStatus, IntegrationError> {
    let executable = integration_executable(id).ok_or(IntegrationError {
        code: "AGENT_NOT_FOUND",
        message: "먼저 해당 AI 코딩 도구를 설치해주세요.",
    })?;
    let broker = ensure_current_broker(app)?;
    let catalog = materialize_catalog(app, &broker, id)?;
    let owned_codex_aliases = (id == AgentIntegrationId::Codex)
        .then(official_codex_marketplace_aliases)
        .unwrap_or_default();
    let owns_existing_installation = marker_version(app, id).is_some()
        || cached_bundle_is_official(id)
        || !owned_codex_aliases.is_empty();

    if id == AgentIntegrationId::Codex && owns_existing_installation {
        refresh_owned_codex_marketplace(
            &executable,
            catalog.as_os_str().to_owned(),
            &owned_codex_aliases,
        )?;
    } else {
        let _ = run_agent_command(
            &executable,
            marketplace_add_args(id, catalog.as_os_str().to_owned()),
        );
        if !install_or_update(&executable, id) {
            return Err(IntegrationError {
                code: "AGENT_INSTALL_FAILED",
                message: "플러그인을 설치하지 못했습니다. 해당 도구의 로그인과 플러그인 정책을 확인해주세요.",
            });
        }

        if !current_bundle_is_cached(id) || !connection_configuration_is_current(app, id, &broker) {
            if !owns_existing_installation && !cached_bundle_is_official(id) {
                return Err(IntegrationError {
                    code: "AGENT_MARKETPLACE_CONFLICT",
                    message: "같은 이름의 다른 Kavranta marketplace가 연결되어 있습니다. 해당 연결을 제거한 뒤 다시 시도해주세요.",
                });
            }
            reconnect_owned_marketplace(&executable, id, catalog.as_os_str().to_owned())?;
            if !refresh_after_marketplace_reconnect(&executable, id) {
                return Err(IntegrationError {
                    code: "AGENT_INSTALL_FAILED",
                    message: "플러그인을 설치하지 못했습니다. 해당 도구의 로그인과 플러그인 정책을 확인해주세요.",
                });
            }
        }
    }
    validate_installed_connection(app, id, &broker)?;
    remove_legacy_codex_plugin(&executable, id);
    persist_marker(app, id)?;
    Ok(status(app, id, Some(&broker), true))
}

fn validate_installed_connection(
    app: &AppHandle,
    id: AgentIntegrationId,
    broker: &Path,
) -> Result<(), IntegrationError> {
    if !current_bundle_is_cached(id) {
        return Err(IntegrationError {
            code: "AGENT_BUNDLE_NOT_UPDATED",
            message: "AI 도구가 새 연동 번들을 적용하지 않았습니다. 기존 marketplace 연결을 확인해주세요.",
        });
    }
    if !connection_configuration_is_current(app, id, broker) {
        return Err(IntegrationError {
            code: "AGENT_CONFIGURATION_NOT_APPLIED",
            message: "AI 도구가 Kavranta broker 설정을 적용하지 않았습니다. 기존 marketplace 연결을 확인해주세요.",
        });
    }
    Ok(())
}

fn status(
    app: &AppHandle,
    id: AgentIntegrationId,
    broker: Option<&Path>,
    catalog_available: bool,
) -> AgentIntegrationStatus {
    let cli_detected = integration_executable(id).is_some();
    let vscode_detected = id == AgentIntegrationId::GithubCopilot && detect_vscode();
    let detected = cli_detected || vscode_detected;
    let installed_version = installed_version(app, id);
    let installed = installed_version.is_some();
    let legacy_version = installed_version
        .as_deref()
        .is_some_and(is_legacy_bundle_version);
    let update_available = installed_version
        .as_deref()
        .is_some_and(|version| is_update_available(version, agent_bundle_version()));
    let configuration_current =
        broker.is_some_and(|broker| connection_configuration_is_current(app, id, broker));
    let needs_repair =
        integration_requires_repair(installed, update_available, configuration_current);
    let action_blocker = action_blocker(cli_detected, broker.is_some(), catalog_available);
    let (protection, detail) =
        integration_detail(id, installed, needs_repair, cli_detected, vscode_detected);

    AgentIntegrationStatus {
        id,
        name: integration_name(id),
        detected,
        installed,
        installed_version,
        legacy_version,
        current_version: agent_bundle_version(),
        update_available,
        needs_repair,
        protection,
        detail,
        can_install: action_blocker.is_none(),
        action_blocker,
    }
}

fn action_blocker(
    cli_detected: bool,
    broker_available: bool,
    catalog_available: bool,
) -> Option<AgentIntegrationBlocker> {
    if !cli_detected {
        Some(AgentIntegrationBlocker::ToolNotFound)
    } else if !broker_available {
        Some(AgentIntegrationBlocker::BrokerUnavailable)
    } else if !catalog_available {
        Some(AgentIntegrationBlocker::BundleUnavailable)
    } else {
        None
    }
}

fn integration_detail(
    id: AgentIntegrationId,
    installed: bool,
    needs_repair: bool,
    cli_detected: bool,
    vscode_detected: bool,
) -> (&'static str, String) {
    match (id, installed, needs_repair, cli_detected, vscode_detected) {
        (_, true, true, _, _) => (
            "inactive",
            "플러그인은 있지만 broker 실행 경로나 감사 기록 설정이 현재 앱과 맞지 않아 복구가 필요합니다.".to_owned(),
        ),
        (AgentIntegrationId::Codex, true, false, _, _) => (
            "broker",
            "Redacted broker가 연결되어 있습니다. 직접 파일 차단 수준은 Codex 권한 프로필에 따라 달라집니다.".to_owned(),
        ),
        (
            AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot,
            true,
            false,
            _,
            _,
        ) => (
            "guarded",
            "공통 Skill, MCP broker, 직접 env 접근 Guard가 연결되어 있습니다.".to_owned(),
        ),
        (AgentIntegrationId::GithubCopilot, false, false, false, true) => (
            "inactive",
            "VS Code는 감지했지만 Copilot CLI가 필요합니다. CLI 설치 후 여기서 한 번에 연결할 수 있습니다.".to_owned(),
        ),
        (_, false, false, true, _) => (
            "inactive",
            "도구를 감지했습니다. Kavranta 연동을 설치할 수 있습니다.".to_owned(),
        ),
        _ => (
            "inactive",
            "도구가 설치되면 Kavranta에서 연동할 수 있습니다.".to_owned(),
        ),
    }
}

fn integration_requires_repair(
    installed: bool,
    update_available: bool,
    configuration_current: bool,
) -> bool {
    installed && !update_available && !configuration_current
}

#[cfg(test)]
mod tests;

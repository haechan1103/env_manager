use semver::Version;
use serde::{Deserialize, Serialize};

pub(super) const PLUGIN_NAME: &str = "env-manager";
pub(super) const MARKETPLACE_NAME: &str = "env-manager";
pub(super) const CODEX_MARKETPLACE_NAME: &str = "env-manager-desktop";
pub(super) const KAVRANTA_REPOSITORY: &str = "https://github.com/haechan1103/kavranta";
pub(super) const LEGACY_ENV_MANAGER_REPOSITORY: &str = "https://github.com/haechan1103/env_manager";
pub(super) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const AGENT_BUNDLE_VERSION: &str = include_str!("../../../plugins/env-manager/VERSION");

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIntegrationId {
    Codex,
    ClaudeCode,
    GithubCopilot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationStatus {
    pub id: AgentIntegrationId,
    pub name: &'static str,
    pub detected: bool,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub legacy_version: bool,
    pub current_version: &'static str,
    pub update_available: bool,
    pub needs_repair: bool,
    pub protection: &'static str,
    pub detail: String,
    pub can_install: bool,
    pub action_blocker: Option<AgentIntegrationBlocker>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIntegrationBlocker {
    ToolNotFound,
    BrokerUnavailable,
    BundleUnavailable,
}

#[derive(Debug)]
pub struct IntegrationError {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstallationMarker {
    #[serde(alias = "version")]
    pub(super) bundle_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexMarketplaceAlias {
    pub(super) name: String,
    pub(super) remove_marketplace: bool,
}

pub(super) fn agent_bundle_version() -> &'static str {
    AGENT_BUNDLE_VERSION.trim()
}

pub(super) fn is_update_available(installed: &str, current: &str) -> bool {
    match (Version::parse(installed), Version::parse(current)) {
        (Ok(installed), Ok(current)) => installed < current,
        _ => installed != current,
    }
}

pub(super) fn is_legacy_bundle_version(version: &str) -> bool {
    Version::parse(version).is_ok_and(|version| version.major == 0)
}

pub(super) fn integration_name(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => "Codex",
        AgentIntegrationId::ClaudeCode => "Claude Code",
        AgentIntegrationId::GithubCopilot => "GitHub Copilot / VS Code",
    }
}

pub(super) fn integration_slug(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => "codex",
        AgentIntegrationId::ClaudeCode => "claude-code",
        AgentIntegrationId::GithubCopilot => "github-copilot",
    }
}

pub(super) fn marketplace_name(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => CODEX_MARKETPLACE_NAME,
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => MARKETPLACE_NAME,
    }
}

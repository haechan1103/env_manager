use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use semver::Version;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::command::run_agent_command;
use super::model::{
    AgentIntegrationId, CODEX_MARKETPLACE_NAME, CodexMarketplaceAlias, InstallationMarker,
    IntegrationError, KAVRANTA_REPOSITORY, LEGACY_ENV_MANAGER_REPOSITORY, MARKETPLACE_NAME,
    PLUGIN_NAME, agent_bundle_version, integration_slug, marketplace_name,
};

pub(super) fn current_bundle_is_cached(id: AgentIntegrationId) -> bool {
    current_cached_bundle(id)
        .map(|(version, _)| version)
        .as_deref()
        == Some(agent_bundle_version())
}

pub(super) fn connection_configuration_is_current(
    app: &AppHandle,
    id: AgentIntegrationId,
    broker: &Path,
) -> bool {
    let Some((version, root)) = current_cached_bundle(id) else {
        return false;
    };
    if version != agent_bundle_version() {
        return false;
    }
    let Ok(app_data) = app.path().app_data_dir() else {
        return false;
    };
    let Ok(mcp) = read_plugin_json(&root.join(".mcp.json")) else {
        return false;
    };
    let Ok(hooks) = read_plugin_json(&root.join("hooks/hooks.json")) else {
        return false;
    };
    connection_files_are_current(&mcp, &hooks, broker, &app_data, id)
}

pub(super) fn connection_files_are_current(
    mcp: &Value,
    hooks: &Value,
    broker: &Path,
    app_data: &Path,
    id: AgentIntegrationId,
) -> bool {
    let server = &mcp["mcpServers"]["env-manager"];
    let expected_audit = app_data
        .join("agent-activity")
        .to_string_lossy()
        .into_owned();
    let expected_app_data = app_data.to_string_lossy().into_owned();
    let expected_broker = broker.to_string_lossy();
    let expected_hook = format!("\"{expected_broker}\" guard-hook");
    server["command"].as_str() == Some(expected_broker.as_ref())
        && server["env"]["ENV_MANAGER_AUDIT_DIR"].as_str() == Some(expected_audit.as_str())
        && server["env"]["ENV_MANAGER_APP_DATA_DIR"].as_str() == Some(expected_app_data.as_str())
        && server["env"]["ENV_MANAGER_AGENT_HOST"].as_str() == Some(integration_slug(id))
        && hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str()
            == Some(expected_hook.as_str())
}

pub(super) fn cached_bundle_is_official(id: AgentIntegrationId) -> bool {
    let Some((_, root)) = cached_bundle(id) else {
        return false;
    };
    let manifest_name = match id {
        AgentIntegrationId::Codex => ".codex-plugin/plugin.json",
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            ".claude-plugin/plugin.json"
        }
    };
    let Ok(manifest) = read_plugin_json(&root.join(manifest_name)) else {
        return false;
    };
    manifest_is_official(&manifest)
}

pub(super) fn manifest_is_official(manifest: &Value) -> bool {
    manifest["name"].as_str() == Some(PLUGIN_NAME)
        && matches!(
            manifest["repository"].as_str(),
            Some(KAVRANTA_REPOSITORY) | Some(LEGACY_ENV_MANAGER_REPOSITORY)
        )
}

pub(super) fn official_codex_marketplace_aliases() -> Vec<CodexMarketplaceAlias> {
    let Some(base) = BaseDirs::new() else {
        return Vec::new();
    };
    official_codex_marketplace_aliases_from_config(&base.home_dir().join(".codex/config.toml"))
}

pub(super) fn official_codex_marketplace_aliases_from_config(
    config_path: &Path,
) -> Vec<CodexMarketplaceAlias> {
    let Ok(source) = fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Ok(config) = toml::from_str::<toml::Table>(&source) else {
        return Vec::new();
    };
    let Some(marketplaces) = config.get("marketplaces").and_then(toml::Value::as_table) else {
        return Vec::new();
    };

    marketplaces
        .iter()
        .filter(|(name, _)| name.as_str() != CODEX_MARKETPLACE_NAME)
        .filter_map(|(name, settings)| {
            let settings = settings.as_table()?;
            if settings.get("source_type").and_then(toml::Value::as_str) != Some("local") {
                return None;
            }
            let root = PathBuf::from(settings.get("source")?.as_str()?);
            Some(CodexMarketplaceAlias {
                name: name.clone(),
                remove_marketplace: codex_marketplace_is_official(&root)?,
            })
        })
        .collect()
}

fn codex_marketplace_is_official(root: &Path) -> Option<bool> {
    let root = fs::canonicalize(root).ok()?;
    let marketplace = read_plugin_json(&root.join(".agents/plugins/marketplace.json")).ok()?;
    let plugins = marketplace["plugins"].as_array()?;
    let plugin = plugins
        .iter()
        .find(|plugin| plugin["name"].as_str() == Some(PLUGIN_NAME))?;
    let source = &plugin["source"];
    let relative = source
        .as_str()
        .or_else(|| source.get("path").and_then(Value::as_str))?;
    if source.is_object() && source.get("source").and_then(Value::as_str) != Some("local") {
        return None;
    }
    let plugin_root = fs::canonicalize(root.join(relative)).ok()?;
    if !plugin_root.starts_with(&root) {
        return None;
    }
    let manifest = read_plugin_json(&plugin_root.join(".codex-plugin/plugin.json")).ok()?;
    manifest_is_official(&manifest).then_some(plugins.len() == 1)
}

pub(super) fn persist_marker(
    app: &AppHandle,
    id: AgentIntegrationId,
) -> Result<(), IntegrationError> {
    let app_data = app.path().app_data_dir().map_err(|_| IntegrationError {
        code: "APP_DATA_UNAVAILABLE",
        message: "앱 데이터 경로를 확인하지 못했습니다.",
    })?;
    let directory = app_data.join("agent-integrations/installations");
    fs::create_dir_all(&directory).map_err(|_| IntegrationError {
        code: "INSTALL_STATE_WRITE_FAILED",
        message: "연동 설치 상태를 저장하지 못했습니다.",
    })?;
    let mut bytes = serde_json::to_vec_pretty(&json!(InstallationMarker {
        bundle_version: agent_bundle_version().to_owned(),
    }))
    .map_err(|_| IntegrationError {
        code: "INSTALL_STATE_WRITE_FAILED",
        message: "연동 설치 상태를 저장하지 못했습니다.",
    })?;
    bytes.push(b'\n');
    fs::write(
        directory.join(format!("{}.json", integration_slug(id))),
        bytes,
    )
    .map_err(|_| IntegrationError {
        code: "INSTALL_STATE_WRITE_FAILED",
        message: "연동 설치 상태를 저장하지 못했습니다.",
    })
}

pub(super) fn installed_version(app: &AppHandle, id: AgentIntegrationId) -> Option<String> {
    cache_version(id).or_else(|| marker_version(app, id))
}

pub(super) fn marker_version(app: &AppHandle, id: AgentIntegrationId) -> Option<String> {
    let path = app
        .path()
        .app_data_dir()
        .ok()?
        .join("agent-integrations/installations")
        .join(format!("{}.json", integration_slug(id)));
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<InstallationMarker>(&bytes)
        .ok()
        .map(|marker| marker.bundle_version)
}

fn cache_version(id: AgentIntegrationId) -> Option<String> {
    cached_bundle(id).map(|(version, _)| version)
}

fn cached_bundle(id: AgentIntegrationId) -> Option<(String, PathBuf)> {
    current_cached_bundle(id).or_else(|| legacy_cached_bundle(id))
}

fn current_cached_bundle(id: AgentIntegrationId) -> Option<(String, PathBuf)> {
    cached_bundle_in_marketplace(id, marketplace_name(id))
}

fn legacy_cached_bundle(id: AgentIntegrationId) -> Option<(String, PathBuf)> {
    (id == AgentIntegrationId::Codex)
        .then(|| cached_bundle_in_marketplace(id, MARKETPLACE_NAME))
        .flatten()
}

fn cached_bundle_in_marketplace(
    id: AgentIntegrationId,
    marketplace: &str,
) -> Option<(String, PathBuf)> {
    let base = BaseDirs::new()?;
    let (root, manifest) = match id {
        AgentIntegrationId::Codex => (
            base.home_dir()
                .join(".codex/plugins/cache")
                .join(marketplace)
                .join(PLUGIN_NAME),
            ".codex-plugin/plugin.json",
        ),
        AgentIntegrationId::ClaudeCode => (
            base.home_dir()
                .join(".claude/plugins/cache")
                .join(marketplace)
                .join(PLUGIN_NAME),
            ".claude-plugin/plugin.json",
        ),
        AgentIntegrationId::GithubCopilot => (
            base.home_dir()
                .join(".copilot/installed-plugins")
                .join(marketplace)
                .join(PLUGIN_NAME),
            ".claude-plugin/plugin.json",
        ),
    };
    newest_manifest_bundle(&root, manifest)
}

pub(super) fn legacy_codex_bundle_is_official() -> bool {
    let Some((_, root)) = legacy_cached_bundle(AgentIntegrationId::Codex) else {
        return false;
    };
    read_plugin_json(&root.join(".codex-plugin/plugin.json"))
        .is_ok_and(|manifest| manifest_is_official(&manifest))
}

fn read_plugin_json(path: &Path) -> Result<Value, IntegrationError> {
    let bytes = fs::read(path).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_UNAVAILABLE",
        message: "플러그인 설정을 읽지 못했습니다.",
    })?;
    serde_json::from_slice(&bytes).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_INVALID",
        message: "플러그인 설정 형식이 올바르지 않습니다.",
    })
}

fn newest_manifest_bundle(root: &Path, manifest: &str) -> Option<(String, PathBuf)> {
    let entries = fs::read_dir(root).ok()?;
    let versions = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(manifest).is_file())
        .filter_map(|entry| {
            let bytes = fs::read(entry.path().join(manifest)).ok()?;
            let version = serde_json::from_slice::<Value>(&bytes)
                .ok()?
                .get("version")?
                .as_str()
                .map(str::to_owned)?;
            Some((version, entry.path()))
        })
        .collect::<Vec<_>>();
    versions.into_iter().max_by(|(left, _), (right, _)| {
        match (Version::parse(left), Version::parse(right)) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        }
    })
}

pub(super) fn remove_legacy_codex_plugin(executable: &Path, id: AgentIntegrationId) {
    if id != AgentIntegrationId::Codex || !legacy_codex_bundle_is_official() {
        return;
    }
    let legacy = format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}");
    let _ = run_agent_command(
        executable,
        vec!["plugin".into(), "remove".into(), legacy.into()],
    );
}

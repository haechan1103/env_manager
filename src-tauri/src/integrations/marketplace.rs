use std::ffi::OsString;
use std::path::Path;

use super::command::run_agent_command;
use super::installation::legacy_codex_bundle_is_official;
use super::model::{
    AgentIntegrationId, CODEX_MARKETPLACE_NAME, CodexMarketplaceAlias, IntegrationError,
    MARKETPLACE_NAME, PLUGIN_NAME, marketplace_name,
};

pub(super) fn marketplace_add_args(_id: AgentIntegrationId, catalog: OsString) -> Vec<OsString> {
    vec!["plugin".into(), "marketplace".into(), "add".into(), catalog]
}

fn marketplace_remove_args(id: AgentIntegrationId) -> Vec<OsString> {
    marketplace_remove_named_args(marketplace_name(id))
}

pub(super) fn marketplace_remove_named_args(name: &str) -> Vec<OsString> {
    vec![
        "plugin".into(),
        "marketplace".into(),
        "remove".into(),
        name.into(),
    ]
}

pub(super) fn reconnect_owned_marketplace(
    executable: &Path,
    id: AgentIntegrationId,
    catalog: OsString,
) -> Result<(), IntegrationError> {
    // Call only after finding an app-owned marker or validating the cached bundle as
    // this project's official plugin. An unrelated marketplace is never removed.
    let _ = run_agent_command(executable, marketplace_remove_args(id));
    if run_agent_command(executable, marketplace_add_args(id, catalog)) {
        Ok(())
    } else {
        Err(IntegrationError {
            code: "AGENT_MARKETPLACE_FAILED",
            message: "AI 도구의 Kavranta marketplace를 연결하지 못했습니다.",
        })
    }
}

pub(super) fn install_or_update(executable: &Path, id: AgentIntegrationId) -> bool {
    run_agent_command(executable, install_args(id))
        || run_agent_command(executable, update_args(id))
}

pub(super) fn refresh_owned_codex_marketplace(
    executable: &Path,
    catalog: OsString,
    legacy_aliases: &[CodexMarketplaceAlias],
) -> Result<(), IntegrationError> {
    let remove_legacy_plugin = legacy_codex_bundle_is_official();
    if refresh_owned_codex_marketplace_with(catalog, legacy_aliases, remove_legacy_plugin, |args| {
        run_agent_command(executable, args)
    }) {
        Ok(())
    } else {
        Err(IntegrationError {
            code: "AGENT_MARKETPLACE_FAILED",
            message: "Codex의 기존 Kavranta 연결을 새 연동 번들로 교체하지 못했습니다.",
        })
    }
}

pub(super) fn refresh_owned_codex_marketplace_with(
    catalog: OsString,
    legacy_aliases: &[CodexMarketplaceAlias],
    remove_legacy_plugin: bool,
    mut run: impl FnMut(Vec<OsString>) -> bool,
) -> bool {
    let id = AgentIntegrationId::Codex;

    // Older builds used a legacy config key and versioned catalog paths. Remove
    // only installations already proven to be app-owned, then add one source.
    let _ = run(remove_args(id));
    if remove_legacy_plugin
        && !legacy_aliases
            .iter()
            .any(|alias| alias.name == MARKETPLACE_NAME)
    {
        let legacy = format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}");
        let _ = run(vec!["plugin".into(), "remove".into(), legacy.into()]);
    }
    for alias in legacy_aliases {
        let selector = format!("{PLUGIN_NAME}@{}", alias.name);
        let _ = run(vec!["plugin".into(), "remove".into(), selector.into()]);
    }
    let _ = run(marketplace_remove_named_args(CODEX_MARKETPLACE_NAME));
    for alias in legacy_aliases
        .iter()
        .filter(|alias| alias.remove_marketplace)
    {
        let _ = run(marketplace_remove_named_args(&alias.name));
    }

    run(marketplace_add_args(id, catalog)) && run(install_args(id))
}

pub(super) fn refresh_after_marketplace_reconnect(
    executable: &Path,
    id: AgentIntegrationId,
) -> bool {
    refresh_after_marketplace_reconnect_with(id, |args| run_agent_command(executable, args))
}

pub(super) fn refresh_after_marketplace_reconnect_with(
    id: AgentIntegrationId,
    mut run: impl FnMut(Vec<OsString>) -> bool,
) -> bool {
    if id == AgentIntegrationId::Codex {
        // Evict the exact stale app-owned cache before reinstalling. A missing
        // installation is harmless because the following add recreates it.
        let _ = run(remove_args(id));
    }
    run(install_args(id)) || run(update_args(id))
}

fn install_args(id: AgentIntegrationId) -> Vec<OsString> {
    let plugin = plugin_selector(id);
    match id {
        AgentIntegrationId::Codex => vec!["plugin".into(), "add".into(), plugin.into()],
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            vec!["plugin".into(), "install".into(), plugin.into()]
        }
    }
}

fn update_args(id: AgentIntegrationId) -> Vec<OsString> {
    let plugin = plugin_selector(id);
    match id {
        AgentIntegrationId::Codex => vec!["plugin".into(), "add".into(), plugin.into()],
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            vec!["plugin".into(), "update".into(), plugin.into()]
        }
    }
}

fn remove_args(id: AgentIntegrationId) -> Vec<OsString> {
    let plugin = plugin_selector(id);
    match id {
        AgentIntegrationId::Codex => vec!["plugin".into(), "remove".into(), plugin.into()],
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            vec!["plugin".into(), "uninstall".into(), plugin.into()]
        }
    }
}

pub(super) fn plugin_selector(id: AgentIntegrationId) -> String {
    format!("{PLUGIN_NAME}@{}", marketplace_name(id))
}

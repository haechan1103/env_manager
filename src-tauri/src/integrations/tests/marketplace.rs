use std::ffi::OsString;

use super::super::marketplace::{
    plugin_selector, refresh_after_marketplace_reconnect_with, refresh_owned_codex_marketplace_with,
};
use super::super::model::{
    AgentIntegrationId, CodexMarketplaceAlias, MARKETPLACE_NAME, marketplace_name,
};

#[test]
fn codex_uses_an_app_owned_marketplace_identity() {
    assert_eq!(
        plugin_selector(AgentIntegrationId::Codex),
        "env-manager@env-manager-desktop"
    );
    assert_eq!(
        marketplace_name(AgentIntegrationId::Codex),
        "env-manager-desktop"
    );
}

#[test]
fn claude_and_copilot_keep_the_shared_marketplace_identity() {
    for id in [
        AgentIntegrationId::ClaudeCode,
        AgentIntegrationId::GithubCopilot,
    ] {
        assert_eq!(plugin_selector(id), "env-manager@env-manager");
    }
}

#[test]
fn codex_repair_evicts_the_exact_plugin_cache_before_reinstalling() {
    let mut commands = Vec::new();
    let refreshed = refresh_after_marketplace_reconnect_with(AgentIntegrationId::Codex, |args| {
        commands.push(args);
        true
    });

    assert!(refreshed);
    assert_eq!(
        commands,
        vec![
            vec![
                OsString::from("plugin"),
                OsString::from("remove"),
                OsString::from("env-manager@env-manager-desktop"),
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("add"),
                OsString::from("env-manager@env-manager-desktop"),
            ],
        ]
    );
}

#[test]
fn codex_repair_continues_when_only_the_stale_cache_remains() {
    let mut commands = Vec::new();
    let refreshed = refresh_after_marketplace_reconnect_with(AgentIntegrationId::Codex, |args| {
        let succeeds = args.get(1).is_some_and(|value| value == "add");
        commands.push(args);
        succeeds
    });

    assert!(refreshed);
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0][1], "remove");
    assert_eq!(commands[1][1], "add");
}

#[test]
fn codex_update_replaces_all_app_owned_marketplace_aliases_before_installing() {
    let catalog = OsString::from("/catalogs/1.9.0/codex");
    let aliases = vec![CodexMarketplaceAlias {
        name: "personal".to_owned(),
        remove_marketplace: true,
    }];
    let mut commands = Vec::new();

    let refreshed =
        refresh_owned_codex_marketplace_with(catalog.clone(), &aliases, false, |args| {
            commands.push(args);
            true
        });

    assert!(refreshed);
    assert_eq!(
        commands,
        vec![
            vec![
                OsString::from("plugin"),
                OsString::from("remove"),
                OsString::from("env-manager@env-manager-desktop"),
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("remove"),
                OsString::from("env-manager@personal"),
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("marketplace"),
                OsString::from("remove"),
                OsString::from("env-manager-desktop"),
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("marketplace"),
                OsString::from("remove"),
                OsString::from("personal"),
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("marketplace"),
                OsString::from("add"),
                catalog,
            ],
            vec![
                OsString::from("plugin"),
                OsString::from("add"),
                OsString::from("env-manager@env-manager-desktop"),
            ],
        ]
    );
}

#[test]
fn codex_update_preserves_legacy_name_after_migration_is_complete() {
    let mut commands = Vec::new();
    let refreshed = refresh_owned_codex_marketplace_with(
        OsString::from("/catalogs/1.6.2/codex"),
        &[],
        false,
        |args| {
            commands.push(args);
            true
        },
    );

    assert!(refreshed);
    assert!(!commands.iter().any(|args| {
        args.get(1).is_some_and(|value| value == "marketplace")
            && args.get(2).is_some_and(|value| value == "remove")
            && args.get(3).is_some_and(|value| value == MARKETPLACE_NAME)
    }));
}

#[test]
fn codex_update_requires_marketplace_and_plugin_install_to_succeed() {
    let mut add_marketplace_attempted = false;
    let refreshed = refresh_owned_codex_marketplace_with(
        OsString::from("/catalogs/1.9.0/codex"),
        &[],
        false,
        |args| {
            if args.get(1).is_some_and(|value| value == "marketplace")
                && args.get(2).is_some_and(|value| value == "add")
            {
                add_marketplace_attempted = true;
                return false;
            }
            true
        },
    );

    assert!(!refreshed);
    assert!(add_marketplace_attempted);
}

#[test]
fn codex_update_removes_only_plugin_from_shared_legacy_marketplace() {
    let aliases = vec![CodexMarketplaceAlias {
        name: "personal".to_owned(),
        remove_marketplace: false,
    }];
    let mut commands = Vec::new();

    assert!(refresh_owned_codex_marketplace_with(
        OsString::from("/catalogs/1.9.2/codex"),
        &aliases,
        false,
        |args| {
            commands.push(args);
            true
        },
    ));
    assert!(commands.iter().any(|args| {
        args == &vec![
            OsString::from("plugin"),
            OsString::from("remove"),
            OsString::from("env-manager@personal"),
        ]
    }));
    assert!(!commands.iter().any(|args| {
        args.get(1).is_some_and(|value| value == "marketplace")
            && args.get(2).is_some_and(|value| value == "remove")
            && args.get(3).is_some_and(|value| value == "personal")
    }));
}

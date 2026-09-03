use std::fs;
use std::path::Path;

use super::super::catalog::{catalog_is_valid, read_json, rewrite_marketplace_name};
use super::super::model::{CODEX_MARKETPLACE_NAME, agent_bundle_version};

#[test]
fn codex_materialized_marketplace_gets_the_app_owned_name() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let marketplace = directory.path().join("marketplace.json");
    fs::write(&marketplace, r#"{"name":"env-manager","plugins":[]}"#).expect("marketplace fixture");

    rewrite_marketplace_name(&marketplace, CODEX_MARKETPLACE_NAME)
        .expect("marketplace name rewrite");

    let rewritten = read_json(&marketplace).expect("rewritten marketplace");
    assert_eq!(rewritten["name"], CODEX_MARKETPLACE_NAME);
}

#[test]
fn catalog_validation_requires_both_agent_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    assert!(catalog_is_valid(root));
}

#[test]
fn agent_bundle_version_is_independent_from_the_app_release() {
    assert_eq!(agent_bundle_version(), "1.9.2");
    assert_ne!(agent_bundle_version(), env!("CARGO_PKG_VERSION"));
}

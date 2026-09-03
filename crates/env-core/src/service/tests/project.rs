use env_test_support::SyntheticProject;

use super::super::*;

#[test]
fn initialization_classifies_without_exposing_values() {
    let project = SyntheticProject::new();
    project.write(
            ".env.local",
            "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\nPORT=fake_3000\nCUSTOM=fake_value\n",
        );
    let service = ProjectService::open(project.root()).expect("service");
    let projection = service.initialize().expect("initialize");

    assert_eq!(projection.files.len(), 1);
    let variables = &projection.files[0].groups[0].variables;
    assert_eq!(variables[0].codex_access, CodexAccess::Protected);
    assert_eq!(variables[1].codex_access, CodexAccess::Unclassified);
    assert_eq!(variables[2].codex_access, CodexAccess::Unclassified);
    assert_eq!(projection.unclassified_count, 2);
    assert_eq!(projection.access_review_count, 0);
    assert!(variables.iter().all(|item| item.display_value.is_none()));
}

#[test]
fn initialization_revokes_only_legacy_heuristic_allows() {
    let project = SyntheticProject::new();
    project.write(".env.local", "PORT=fake_3000\nHOST=fake_localhost\n");
    let service = ProjectService::open(project.root()).expect("service");
    let store = ManifestStore::for_root(project.root());
    let mut manifest = store.load().expect("manifest");
    manifest.variables.insert(
        "PORT".to_owned(),
        VariablePolicy {
            codex_access: CodexAccess::ReadWrite,
            classified_by: ClassificationSource::Heuristic,
        },
    );
    manifest.variables.insert(
        "HOST".to_owned(),
        VariablePolicy {
            codex_access: CodexAccess::ReadWrite,
            classified_by: ClassificationSource::User,
        },
    );
    store.save(&manifest).expect("seed manifest");

    let projection = service.initialize().expect("initialize");
    let policies = ManifestStore::for_root(project.root())
        .load()
        .expect("migrated manifest");

    assert_eq!(policies.access_for("PORT"), CodexAccess::Unclassified);
    assert_eq!(policies.access_for("HOST"), CodexAccess::ReadWrite);
    assert_eq!(projection.unclassified_count, 1);
}

#[test]
fn public_secret_name_is_the_only_name_based_access_review_exception() {
    let project = SyntheticProject::new();
    project.write(
        ".env.local",
        "NEXT_PUBLIC_API_KEY=fake_secret\nAPI_KEY=fake_secret\nCUSTOM_MODE=fake_value\n",
    );
    let service = ProjectService::open(project.root()).expect("service");

    let projection = service.initialize().expect("initialize");
    let requiring_review = projection
        .classification_review
        .iter()
        .filter(|item| !item.review_reasons.is_empty())
        .collect::<Vec<_>>();

    assert_eq!(projection.access_review_count, 1);
    assert_eq!(requiring_review[0].key, "NEXT_PUBLIC_API_KEY");
    assert_eq!(
        requiring_review[0].review_reasons,
        vec![ClassificationReviewReason::ClientExposureConflict]
    );
}

#[test]
fn local_file_display_name_changes_projection_without_renaming_the_file() {
    let project = SyntheticProject::new();
    project.write("apps/web/.env.local", "PORT=fake_3000\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    let path = service
        .validate_file_for_display_name("apps/web/.env.local")
        .expect("valid display target");
    let projection = service
        .initialize_with_file_labels(&BTreeMap::from([(path, "Web local".to_owned())]))
        .expect("scan");
    assert_eq!(projection.files[0].display_name, "Web local");
    assert_eq!(projection.files[0].path, "apps/web/.env.local");
    assert!(project.root().join("apps/web/.env.local").is_file());
    assert!(!project.root().join("apps/web/Web local").exists());
}

#[test]
fn scan_classifies_newly_added_names() {
    let project = SyntheticProject::new();
    project.write(".env", "PORT=fake_3000\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    project.write(".env", "PORT=fake_3000\nNEW_CLIENT_SECRET=fake_secret\n");

    let projection = service.scan().expect("scan");
    let variables = &projection.files[0].groups[0].variables;
    assert_eq!(variables[1].codex_access, CodexAccess::Protected);
}

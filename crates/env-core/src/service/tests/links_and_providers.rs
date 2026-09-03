use env_test_support::SyntheticProject;

use super::super::*;

#[test]
fn linked_save_updates_all_members() {
    let project = SyntheticProject::new();
    project.write(".env.local", "PORT=fake_3000\n");
    project.write(".env.development", "PORT=\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .create_link(LinkRequest {
            key: "PORT".to_owned(),
            files: vec![".env.local".to_owned(), ".env.development".to_owned()],
            source_file: None,
        })
        .expect("link");
    service
        .save_value(SaveValueRequest {
            file: ".env.local".to_owned(),
            key: "PORT".to_owned(),
            new_value: "fake_4000".to_owned(),
        })
        .expect("save");

    assert_eq!(project.read(".env.local"), b"PORT=fake_4000\n");
    assert_eq!(project.read(".env.development"), b"PORT=fake_4000\n");
}

#[test]
fn copies_a_protected_value_between_projects_without_returning_it() {
    let source = SyntheticProject::new();
    let target = SyntheticProject::new();
    let canary = "fake_CROSS_PROJECT_CANARY_41";
    source.write(".env.local", &format!("GEMINI_API_KEY={canary}\n"));
    target.write(".env.local", "GEMINI_API_KEY=\n");
    target.write(".env.development", "GEMINI_API_KEY=\n");
    let source_service = ProjectService::open(source.root()).expect("source service");
    let target_service = ProjectService::open(target.root()).expect("target service");
    source_service.initialize().expect("source initialize");
    target_service.initialize().expect("target initialize");
    target_service
        .create_link(LinkRequest {
            key: "GEMINI_API_KEY".to_owned(),
            files: vec![".env.local".to_owned(), ".env.development".to_owned()],
            source_file: None,
        })
        .expect("target link");

    let candidates = source_service
        .redacted_occurrences("GEMINI_API_KEY")
        .expect("redacted candidates");
    let serialized = serde_json::to_string(&candidates).expect("serialize candidates");
    assert!(!serialized.contains(canary));
    assert_eq!(candidates[0].value_state, RedactedValueState::Present);

    let summary = target_service
        .copy_value_from(
            &source_service,
            OpaqueValueCopyRequest {
                source_file: ".env.local".to_owned(),
                target_file: ".env.local".to_owned(),
                key: "GEMINI_API_KEY".to_owned(),
            },
        )
        .expect("opaque copy");

    assert_eq!(
        summary.affected_files,
        vec![".env.development".to_owned(), ".env.local".to_owned()]
    );
    assert_eq!(
        target.read(".env.local"),
        format!("GEMINI_API_KEY={canary}\n").as_bytes()
    );
    assert_eq!(
        target.read(".env.development"),
        format!("GEMINI_API_KEY={canary}\n").as_bytes()
    );
    assert!(
        !serde_json::to_string(&summary)
            .expect("serialize summary")
            .contains(canary)
    );
}

#[test]
fn provider_values_require_managed_unique_non_empty_names() {
    let project = SyntheticProject::new();
    project.write(
        ".env.production",
        "API_KEY=fake_provider_secret\nAPI_HOST=fake_api_host\nEMPTY=\n",
    );
    project.write("notes.env", "OTHER=fake_other\n");
    project.write("notes.txt", "UNMANAGED=fake_unmanaged\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    let selected = service
        .provider_values(
            ".env.production",
            &["API_KEY".to_owned(), "API_HOST".to_owned()],
        )
        .expect("provider values");
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].key(), "API_KEY");
    assert_eq!(selected[0].value(), "fake_provider_secret");

    assert!(
        service
            .provider_values(".env.production", &["EMPTY".to_owned()])
            .is_err()
    );
    assert!(
        service
            .provider_values(
                ".env.production",
                &["API_KEY".to_owned(), "API_KEY".to_owned()],
            )
            .is_err()
    );
    assert_eq!(
        service
            .provider_values("notes.env", &["OTHER".to_owned()])
            .expect("suffix env value")[0]
            .key(),
        "OTHER"
    );
    assert!(
        service
            .provider_values("notes.txt", &["UNMANAGED".to_owned()])
            .is_err()
    );
}

#[test]
fn link_supports_four_peer_occurrences() {
    let project = SyntheticProject::new();
    project.write(".env", "PORT=fake_3000\n");
    project.write(".env.local", "PORT=\n");
    project.write(".env.dev", "PORT=\n");
    project.write("apps/web/.env.local", "PORT=\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    let files = vec![
        ".env".to_owned(),
        ".env.local".to_owned(),
        ".env.dev".to_owned(),
        "apps/web/.env.local".to_owned(),
    ];
    let summary = service
        .create_link(LinkRequest {
            key: "PORT".to_owned(),
            files: files.clone(),
            source_file: Some(".env".to_owned()),
        })
        .expect("link");
    assert_eq!(summary.affected_files.len(), 4);
    service
        .save_value(SaveValueRequest {
            file: ".env.dev".to_owned(),
            key: "PORT".to_owned(),
            new_value: "fake_4100".to_owned(),
        })
        .expect("save");
    for file in files {
        assert_eq!(project.read(&file), b"PORT=fake_4100\n");
    }
}

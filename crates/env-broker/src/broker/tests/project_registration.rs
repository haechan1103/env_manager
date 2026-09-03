use super::*;

#[test]
fn current_workspace_registration_is_redacted_and_immediately_inspectable() {
    let project = SyntheticProject::new();
    project.write(
        ".env.local",
        &format!("DATABASE_PASSWORD={CANARY}\nPUBLIC_PORT=fake_3000\n"),
    );
    let original = project.read(".env.local");
    let app_data = tempfile::tempdir().expect("app data");
    let broker = Broker::with_workspace_and_app_data(
        project.root().to_path_buf(),
        app_data.path().to_path_buf(),
    );

    let plan = broker
        .call_tool("plan_register_current_project", json!({}))
        .expect("registration plan");
    assert!(!plan.to_string().contains(CANARY));
    let plan_id = plan["planId"].as_str().expect("plan id");
    let applied = broker
        .call_tool("apply_plan", json!({ "planId": plan_id }))
        .expect("registration apply");
    assert_eq!(applied["registered"], true);
    assert!(!applied.to_string().contains(CANARY));

    let inspected = broker
        .call_tool(
            "inspect_project",
            json!({ "projectPath": project.root().to_string_lossy() }),
        )
        .expect("registered project inspection");
    assert!(!inspected.to_string().contains(CANARY));
    assert_eq!(project.read(".env.local"), original);

    let registry =
        env_registry::read(&app_data.path().join("projects.json")).expect("local registry");
    assert_eq!(registry.projects.len(), 1);
    assert_eq!(registry.projects[0].id, applied["projectId"]);
    let audit = fs::read_to_string(
        app_data
            .path()
            .join("agent-activity")
            .join(format!("{}.jsonl", registry.projects[0].id)),
    )
    .expect("registration audit");
    assert!(audit.contains("register_current_project"));
    assert!(!audit.contains(CANARY));
}

#[test]
fn registration_repairs_a_missing_manifest_without_overwriting_local_aliases() {
    let project = SyntheticProject::new();
    project.write(".env.local", &format!("DATABASE_PASSWORD={CANARY}\n"));
    let service = ProjectService::open(project.root()).expect("service");
    let app_data = tempfile::tempdir().expect("app data");
    let registry_path = app_data.path().join("projects.json");
    env_registry::write(
        &registry_path,
        &env_registry::RegistryData {
            projects: vec![ProjectRegistration {
                id: service.project_id().to_owned(),
                name: "My local alias".to_owned(),
                display_path: service.root().to_string_lossy().into_owned(),
                root: service.root().to_path_buf(),
                file_labels: Default::default(),
            }],
            last_selected_project_id: Some(service.project_id().to_owned()),
            ..env_registry::RegistryData::default()
        },
    )
    .expect("registry without manifest");
    let broker = Broker::with_workspace_and_app_data(
        project.root().to_path_buf(),
        app_data.path().to_path_buf(),
    );

    let plan = broker
        .call_tool("plan_register_current_project", json!({}))
        .expect("repair plan");
    let plan_id = plan["planId"].as_str().expect("plan id");
    broker
        .call_tool("apply_plan", json!({ "planId": plan_id }))
        .expect("repair apply");

    assert!(project.root().join(env_core::MANIFEST_FILE_NAME).is_file());
    let registry = env_registry::read(&registry_path).expect("repaired registry");
    assert_eq!(registry.projects[0].name, "My local alias");
    assert_eq!(
        registry.last_selected_project_id.as_deref(),
        Some(service.project_id())
    );
    assert!(
        !serde_json::to_string(&registry)
            .expect("registry json")
            .contains(CANARY)
    );
}

#[test]
fn manifest_without_active_registration_is_rejected() {
    let (project, _) = registered_project();
    let error = Broker::with_registered_roots(Vec::new())
        .call_tool(
            "inspect_project",
            json!({ "projectPath": project.root().to_string_lossy() }),
        )
        .expect_err("must reject");
    assert_eq!(error.code(), EnvErrorCode::UnregisteredProject);
}

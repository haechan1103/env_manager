use super::*;

#[test]
fn opaque_stdin_plan_updates_protected_linked_values_without_exposing_them() {
    let project = SyntheticProject::new();
    project.write(".env.local", "AUTH_SECRET=fake_old_secret\n");
    project.write(".env.staging", "AUTH_SECRET=fake_old_secret\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .create_link(LinkRequest {
            key: "AUTH_SECRET".to_owned(),
            files: vec![".env.local".to_owned(), ".env.staging".to_owned()],
            source_file: None,
        })
        .expect("link");
    assert_eq!(
        service.codex_access("AUTH_SECRET").expect("access"),
        CodexAccess::Protected
    );
    let app_data = tempfile::tempdir().expect("app data");
    let registry_path = register_for_stdin_apply(app_data.path(), &service);
    let broker = Broker::with_registered_roots_and_app_data(
        vec![service.root().to_path_buf()],
        app_data.path().to_path_buf(),
    );
    broker.identify_client("codex");

    let plan = broker
        .call_tool(
            "plan_stdin_value_write",
            json!({
                "projectPath": project.root(),
                "file": ".env.local",
                "key": "AUTH_SECRET",
                "trimFinalNewline": true
            }),
        )
        .expect("stdin plan");
    let plan_id = plan["planId"].as_str().expect("plan id");
    assert_eq!(plan["affectedFiles"].as_array().map(Vec::len), Some(2));
    assert!(!plan.to_string().contains("fake_old_secret"));
    let stored_plan_path = app_data
        .path()
        .join("stdin-value-plans")
        .join(format!("{plan_id}.json"));
    let stored_plan = fs::read_to_string(&stored_plan_path).expect("stored stdin plan");
    assert!(!stored_plan.contains("fake_old_secret"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(&stored_plan_path)
                .expect("plan metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    let new_canary = "fake_new_stdin_canary_82a1\n";
    let result = stdin_value::apply_plan(
        app_data.path(),
        &registry_path,
        plan_id,
        true,
        new_canary.as_bytes(),
    )
    .expect("stdin apply");
    let serialized = serde_json::to_string(&result).expect("serialize result");
    assert!(!serialized.contains("fake_new_stdin_canary_82a1"));
    assert_eq!(
        project.read(".env.local"),
        b"AUTH_SECRET=fake_new_stdin_canary_82a1\n"
    );
    assert_eq!(
        project.read(".env.staging"),
        b"AUTH_SECRET=fake_new_stdin_canary_82a1\n"
    );
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            plan_id,
            true,
            &b"fake_replay"[..]
        )
        .expect_err("single use")
        .code(),
        "PLAN_EXPIRED"
    );
    let audit = fs::read_to_string(
        app_data
            .path()
            .join("agent-activity")
            .join(format!("{}.jsonl", service.project_id())),
    )
    .expect("stdin audit");
    assert!(audit.contains("apply_stdin_value"));
    assert!(!audit.contains("fake_new_stdin_canary_82a1"));
    assert!(!audit.contains("fake_old_secret"));
}

#[test]
fn opaque_stdin_plan_rejects_stale_files_and_consumes_the_plan() {
    let project = SyntheticProject::new();
    project.write(".env.local", "AUTH_SECRET=fake_before\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    let app_data = tempfile::tempdir().expect("app data");
    let registry_path = register_for_stdin_apply(app_data.path(), &service);
    let broker = Broker::with_registered_roots_and_app_data(
        vec![service.root().to_path_buf()],
        app_data.path().to_path_buf(),
    );
    let plan = broker
        .call_tool(
            "plan_stdin_value_write",
            json!({
                "projectPath": project.root(),
                "file": ".env.local",
                "key": "AUTH_SECRET"
            }),
        )
        .expect("stdin plan");
    let plan_id = plan["planId"].as_str().expect("plan id");
    project.write(".env.local", "AUTH_SECRET=fake_external_change_longer\n");

    let error = stdin_value::apply_plan(
        app_data.path(),
        &registry_path,
        plan_id,
        false,
        &b"fake_should_not_apply"[..],
    )
    .expect_err("stale plan");
    assert_eq!(error.code(), "FILE_CHANGED_EXTERNALLY");
    assert_eq!(
        project.read(".env.local"),
        b"AUTH_SECRET=fake_external_change_longer\n"
    );
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            plan_id,
            false,
            &b"fake_replay"[..]
        )
        .expect_err("consumed stale plan")
        .code(),
        "PLAN_EXPIRED"
    );
}

#[test]
fn opaque_stdin_plan_rejects_empty_oversized_and_mismatched_input() {
    let project = SyntheticProject::new();
    project.write(".env.local", "AUTH_SECRET=fake_before\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    let app_data = tempfile::tempdir().expect("app data");
    let registry_path = register_for_stdin_apply(app_data.path(), &service);
    let broker = Broker::with_registered_roots_and_app_data(
        vec![service.root().to_path_buf()],
        app_data.path().to_path_buf(),
    );
    let create_plan = |trim_final_newline| {
        broker
            .call_tool(
                "plan_stdin_value_write",
                json!({
                    "projectPath": project.root(),
                    "file": ".env.local",
                    "key": "AUTH_SECRET",
                    "trimFinalNewline": trim_final_newline
                }),
            )
            .expect("stdin plan")["planId"]
            .as_str()
            .expect("plan id")
            .to_owned()
    };

    let empty_plan = create_plan(true);
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &empty_plan,
            true,
            &b"\n"[..]
        )
        .expect_err("empty after trim")
        .code(),
        "STDIN_VALUE_EMPTY"
    );

    let oversized_plan = create_plan(false);
    let oversized = vec![b'x'; 64 * 1024 + 1];
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &oversized_plan,
            false,
            oversized.as_slice()
        )
        .expect_err("oversized")
        .code(),
        "STDIN_VALUE_TOO_LARGE"
    );

    let mismatch_plan = create_plan(true);
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &mismatch_plan,
            false,
            &b"fake_not_applied"[..]
        )
        .expect_err("normalization mismatch")
        .code(),
        "STDIN_NORMALIZATION_MISMATCH"
    );

    let nul_plan = create_plan(false);
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &nul_plan,
            false,
            &b"fake_nul\0value"[..]
        )
        .expect_err("NUL")
        .code(),
        "STDIN_VALUE_INVALID"
    );

    let utf8_plan = create_plan(false);
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &utf8_plan,
            false,
            &[0xff_u8][..]
        )
        .expect_err("invalid UTF-8")
        .code(),
        "STDIN_VALUE_INVALID_UTF8"
    );

    let expired_plan = create_plan(false);
    let expired_path = app_data
        .path()
        .join("stdin-value-plans")
        .join(format!("{expired_plan}.json"));
    let mut expired_json: Value =
        serde_json::from_slice(&fs::read(&expired_path).expect("read expiring plan"))
            .expect("expiring plan JSON");
    expired_json["expiresAtMs"] = json!(0);
    fs::write(
        &expired_path,
        serde_json::to_vec(&expired_json).expect("expired plan JSON"),
    )
    .expect("expire plan");
    assert_eq!(
        stdin_value::apply_plan(
            app_data.path(),
            &registry_path,
            &expired_plan,
            false,
            &b"fake_not_applied"[..]
        )
        .expect_err("expired")
        .code(),
        "PLAN_EXPIRED"
    );
    assert_eq!(project.read(".env.local"), b"AUTH_SECRET=fake_before\n");
}

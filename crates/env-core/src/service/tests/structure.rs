use env_test_support::SyntheticProject;

use super::super::*;

#[test]
fn adds_variable_inside_existing_group_without_duplicate_marker() {
    let project = SyntheticProject::new();
    project.write(
        ".env.local",
        "# @group GPT\nGPT_MODEL=fake_model\n\n# @group App\nPORT=fake_3000\n",
    );
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .add_variable(AddVariableRequest {
            file: ".env.local".to_owned(),
            key: "GPT_TIMEOUT".to_owned(),
            group: "GPT".to_owned(),
            description: vec!["fake timeout description".to_owned()],
            value: "fake_30".to_owned(),
        })
        .expect("add");

    let output = String::from_utf8(project.read(".env.local")).expect("utf8");
    assert_eq!(output.matches("# @group GPT").count(), 1);
    assert!(
        output.find("GPT_TIMEOUT").expect("new key")
            < output.find("# @group App").expect("next group")
    );
}

#[test]
fn creates_and_renames_an_explicit_empty_group() {
    let project = SyntheticProject::new();
    project.write(".env", "PORT=fake_3000\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    service
        .create_group(CreateGroupRequest {
            file: ".env".to_owned(),
            name: "GPT".to_owned(),
        })
        .expect("create group");
    assert_eq!(project.read(".env"), b"PORT=fake_3000\n\n# @group GPT\n");

    service
        .rename_group(RenameGroupRequest {
            file: ".env".to_owned(),
            current_name: "GPT".to_owned(),
            new_name: "OpenAI".to_owned(),
        })
        .expect("rename group");
    assert_eq!(project.read(".env"), b"PORT=fake_3000\n\n# @group OpenAI\n");
}

#[test]
fn creates_group_in_bom_only_file_without_leading_blank_lines() {
    let project = SyntheticProject::new();
    project.write(".env", "\u{feff}");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .create_group(CreateGroupRequest {
            file: ".env".to_owned(),
            name: "GPT".to_owned(),
        })
        .expect("create group");

    assert_eq!(project.read(".env"), b"\xEF\xBB\xBF# @group GPT\n");
}

#[test]
fn refuses_ambiguous_or_reserved_group_names() {
    let project = SyntheticProject::new();
    project.write(".env", "# @group GPT\nA=fake_a\n# @group GPT\nB=fake_b\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    let duplicate = service
        .rename_group(RenameGroupRequest {
            file: ".env".to_owned(),
            current_name: "GPT".to_owned(),
            new_name: "OpenAI".to_owned(),
        })
        .expect_err("duplicate group must be ambiguous");
    assert!(duplicate.to_string().contains("여러 번"));

    let reserved = service
        .create_group(CreateGroupRequest {
            file: ".env".to_owned(),
            name: "기타".to_owned(),
        })
        .expect_err("virtual group name is reserved");
    assert!(reserved.to_string().contains("올바르지"));
}

#[test]
fn adds_ungrouped_variable_before_the_first_group_marker() {
    let project = SyntheticProject::new();
    project.write(".env", "# @group GPT\nGPT_MODEL=fake_model\n");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .add_variable(AddVariableRequest {
            file: ".env".to_owned(),
            key: "PORT".to_owned(),
            group: "기타".to_owned(),
            description: Vec::new(),
            value: "fake_3000".to_owned(),
        })
        .expect("add ungrouped");

    assert_eq!(
        project.read(".env"),
        b"PORT=fake_3000\n# @group GPT\nGPT_MODEL=fake_model\n"
    );
}

#[test]
fn deletes_assignment_and_attached_description_only() {
    let project = SyntheticProject::new();
    project.write(
        ".env",
        "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\n# keep\nPORT=fake_3000\n",
    );
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .delete_variable(DeleteVariableRequest {
            file: ".env".to_owned(),
            key: "GPT_API_KEY".to_owned(),
        })
        .expect("delete");
    assert_eq!(
        project.read(".env"),
        b"# @group GPT\n# keep\nPORT=fake_3000\n"
    );
}

#[test]
fn moves_assignment_with_description_without_reading_value() {
    let project = SyntheticProject::new();
    project.write(
        ".env",
        "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\n# @group App\nPORT=fake_3000\n",
    );
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .move_variable(MoveVariableRequest {
            file: ".env".to_owned(),
            key: "GPT_API_KEY".to_owned(),
            target_group: "App".to_owned(),
        })
        .expect("move");
    assert_eq!(
            project.read(".env"),
            b"# @group GPT\n# @group App\nPORT=fake_3000\n# fake description\nGPT_API_KEY=fake_secret\n"
        );
}

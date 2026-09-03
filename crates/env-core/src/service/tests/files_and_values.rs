use env_test_support::SyntheticProject;

use super::super::*;

#[test]
fn creates_empty_env_file_only_inside_an_existing_project_directory() {
    let project = SyntheticProject::new();
    fs::create_dir_all(project.root().join("apps/mobile")).expect("fixture directory");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    service
        .create_env_file(CreateEnvFileRequest {
            file: "apps/mobile/.env".to_owned(),
        })
        .expect("create env file");
    service
        .create_env_file(CreateEnvFileRequest {
            file: "apps/mobile/.dev.vars.staging".to_owned(),
        })
        .expect("create Wrangler env file");

    assert_eq!(project.read("apps/mobile/.env"), b"");
    assert_eq!(project.read("apps/mobile/.dev.vars.staging"), b"");
    let projection = service.scan().expect("scan");
    assert_eq!(
        projection
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["apps/mobile/.dev.vars.staging", "apps/mobile/.env"]
    );
}

#[test]
fn refuses_example_overwrite_and_path_escape_for_new_env_files() {
    let project = SyntheticProject::new();
    project.write(".env", "PORT=fake_3000\n");
    fs::create_dir_all(project.root().join("node_modules/pkg"))
        .expect("excluded fixture directory");
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");

    for file in [".env.example", ".env", "../.env", "node_modules/pkg/.env"] {
        assert!(
            service
                .create_env_file(CreateEnvFileRequest {
                    file: file.to_owned(),
                })
                .is_err(),
            "must reject {file}"
        );
    }
    assert_eq!(project.read(".env"), b"PORT=fake_3000\n");
}

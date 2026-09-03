use std::io::{Read, Write};
use std::net::TcpListener;

use env_test_support::SyntheticProject;

use super::*;

const CANARY: &str = "fake_CANARY_never_in_projection_7f91";

fn registered_project() -> (SyntheticProject, ProjectService) {
    let project = SyntheticProject::new();
    project.write(
        ".env.local",
        &format!("GPT_API_KEY={CANARY}\nPORT=fake_3000\n"),
    );
    let service = ProjectService::open(project.root()).expect("service");
    service.initialize().expect("initialize");
    service
        .set_codex_access("PORT", CodexAccess::ReadWrite)
        .expect("explicitly allow synthetic runtime setting");
    (project, service)
}

fn register_for_stdin_apply(app_data: &Path, service: &ProjectService) -> PathBuf {
    let registry_path = app_data.join("projects.json");
    env_registry::write(
        &registry_path,
        &env_registry::RegistryData {
            projects: vec![ProjectRegistration {
                id: service.project_id().to_owned(),
                name: "Synthetic".to_owned(),
                display_path: service.root().to_string_lossy().into_owned(),
                root: service.root().to_path_buf(),
                file_labels: Default::default(),
            }],
            ..env_registry::RegistryData::default()
        },
    )
    .expect("stdin registry");
    registry_path
}

mod audit_guard;
mod env_tools;
mod project_registration;
mod provider_tools;
#[path = "stdin_value.rs"]
mod stdin_workflow;

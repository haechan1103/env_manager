use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use env_core::{
    ClassificationReviewReason, ClassificationSource, DiscoveryOptions, EnvError, EnvErrorCode,
    EnvResult, MigrationPlan, MigrationPreview, MutationSummary, ProjectService, TeamImportPlan,
    TeamImportPreview, TeamImportSummary, TeamImportValueSide, is_env_candidate,
};
use env_registry::{ProjectRegistration, RegistryData};
use notify::{
    Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

mod plans;
mod project_data;
mod registry;
mod watcher;

mod agent_activity;
mod credentials;
mod team_channels;

pub use credentials::CredentialRuntime;
pub use team_channels::TeamChannelProjection;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub display_path: String,
}

pub use env_registry::ProviderPushReceipt;

pub struct AppRuntime {
    registry_path: PathBuf,
    audit_dir: PathBuf,
    registry: Mutex<RegistryData>,
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    migration_plans: Mutex<HashMap<String, StoredMigration>>,
    team_import_plans: Mutex<HashMap<String, StoredTeamImport>>,
    next_plan_id: AtomicU64,
}

struct StoredMigration {
    project_id: String,
    expires_at: Instant,
    plan: MigrationPlan,
}

struct StoredTeamImport {
    project_id: String,
    expires_at: Instant,
    plan: TeamImportPlan,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlanProjection {
    pub plan_id: String,
    pub expires_in_seconds: u64,
    pub preview: MigrationPreview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportPlanProjection {
    pub plan_id: String,
    pub expires_in_seconds: u64,
    pub preview: TeamImportPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentActivityEvent {
    pub timestamp_ms: u64,
    pub project_id: String,
    pub actor: String,
    pub category: String,
    pub operation: String,
    pub relative_paths: Vec<String>,
    pub variable_names: Vec<String>,
    pub policy_decision: String,
    pub outcome: String,
    pub result_code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedFilesChanged {
    project_id: String,
    paths: Vec<String>,
}

fn should_rescan_for_event(
    root: &Path,
    path: &Path,
    kind: &EventKind,
    managed_paths: &BTreeSet<PathBuf>,
    ignored_directories: &BTreeSet<String>,
) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    if relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return false;
    }
    if relative.components().any(|component| {
        ignored_directories.contains(component.as_os_str().to_string_lossy().as_ref())
    }) {
        return false;
    }
    if managed_paths.contains(path) {
        return true;
    }
    if path
        .file_name()
        .is_some_and(|name| is_env_candidate(&name.to_string_lossy()))
    {
        return true;
    }
    matches!(
        kind,
        EventKind::Create(CreateKind::Folder)
            | EventKind::Remove(RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Name(_))
    )
}

fn migrate_legacy_file_labels(
    registry_path: &Path,
    registry: &mut RegistryData,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut changed = false;
    let mut migrated_roots = Vec::new();
    for project in &mut registry.projects {
        let Ok(service) = ProjectService::open(&project.root) else {
            continue;
        };
        let Ok(manifest) = env_core::ManifestStore::for_root(service.root()).load() else {
            continue;
        };
        if manifest.file_labels.is_empty() {
            continue;
        }
        for (path, label) in &manifest.file_labels {
            project
                .file_labels
                .entry(path.clone())
                .or_insert_with(|| label.clone());
        }
        changed = true;
        migrated_roots.push(service.root().to_path_buf());
    }
    if changed {
        persist_registry(registry_path, registry)?;
        for root in migrated_roots {
            env_core::ManifestStore::for_root(&root).take_legacy_file_labels()?;
        }
    }
    Ok(())
}

fn migrate_registration_labels(
    project: &mut ProjectRegistration,
    service: &ProjectService,
) -> EnvResult<bool> {
    let labels = env_core::ManifestStore::for_root(service.root())
        .load()?
        .file_labels;
    let migrated = !labels.is_empty();
    for (path, label) in labels {
        project.file_labels.entry(path).or_insert(label);
    }
    Ok(migrated)
}

fn persist_registry(path: &Path, registry: &RegistryData) -> EnvResult<()> {
    env_registry::write(path, registry)
}

fn team_import_plan_expired() -> EnvError {
    EnvError::new(
        EnvErrorCode::PlanExpired,
        "공유 파일 적용 계획이 만료되었습니다. 다시 열어주세요.",
    )
}

impl From<&ProjectRegistration> for ProjectSummary {
    fn from(registration: &ProjectRegistration) -> Self {
        Self {
            id: registration.id.clone(),
            name: registration.name.clone(),
            display_path: registration.display_path.clone(),
        }
    }
}

fn to_relative_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_rescans_only_for_managed_candidates_and_directory_topology() {
        let project = tempfile::tempdir().expect("project");
        let root = project.path().canonicalize().expect("canonical project");
        let managed = root.join(".env.local");
        fs::write(&managed, "PORT=fake_3000\n").expect("managed fixture");
        let managed_paths = BTreeSet::from([managed.clone()]);
        let ignored = DiscoveryOptions::default().ignored_directories;

        assert!(should_rescan_for_event(
            &root,
            &managed,
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(should_rescan_for_event(
            &root,
            &root.join("secrets/runtime.env"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(should_rescan_for_event(
            &root,
            &root.join("workers/api/.dev.vars.staging"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(should_rescan_for_event(
            &root,
            &root.join("apps/new-service"),
            &EventKind::Create(CreateKind::Folder),
            &managed_paths,
            &ignored,
        ));
        assert!(!should_rescan_for_event(
            &root,
            &root.join("README.md"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(!should_rescan_for_event(
            &root,
            &root.join("node_modules/package/runtime.env"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(!should_rescan_for_event(
            &root,
            &root.join("sample.env"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
        assert!(!should_rescan_for_event(
            &root,
            &root.join("../outside/runtime.env"),
            &EventKind::Any,
            &managed_paths,
            &ignored,
        ));
    }

    #[test]
    fn persists_the_last_selected_project_and_falls_back_when_removed() {
        let app_data = tempfile::tempdir().expect("app data");
        let first_root = tempfile::tempdir().expect("first project");
        let second_root = tempfile::tempdir().expect("second project");
        let first_service = ProjectService::open(first_root.path()).expect("first service");
        let second_service = ProjectService::open(second_root.path()).expect("second service");
        let first_id = first_service.project_id().to_owned();
        let second_id = second_service.project_id().to_owned();
        let registry_path = app_data.path().join("projects.json");
        let registry = RegistryData {
            projects: vec![
                ProjectRegistration {
                    id: first_id.clone(),
                    name: "First".to_owned(),
                    display_path: first_root.path().to_string_lossy().into_owned(),
                    root: first_root.path().to_path_buf(),
                    file_labels: BTreeMap::new(),
                },
                ProjectRegistration {
                    id: second_id.clone(),
                    name: "Surgery".to_owned(),
                    display_path: second_root.path().to_string_lossy().into_owned(),
                    root: second_root.path().to_path_buf(),
                    file_labels: BTreeMap::new(),
                },
            ],
            last_selected_project_id: None,
            team_channels: Vec::new(),
            provider_push_receipts: Vec::new(),
        };
        persist_registry(&registry_path, &registry).expect("persist registry");
        let runtime = AppRuntime {
            registry_path: registry_path.clone(),
            audit_dir: app_data.path().join("agent-activity"),
            registry: Mutex::new(registry),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        };

        runtime
            .remember_selected_project(Some(&second_id))
            .expect("remember selection");
        assert_eq!(runtime.last_selected_project_id(), Some(second_id.clone()));
        let saved = fs::read_to_string(&registry_path).expect("saved registry");
        assert!(saved.contains(&format!("\"lastSelectedProjectId\": \"{second_id}\"")));

        runtime.remove(&second_id).expect("remove selected project");
        assert_eq!(runtime.last_selected_project_id(), Some(first_id));
    }

    #[test]
    fn refreshes_projects_registered_by_the_broker_without_restarting() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = tempfile::tempdir().expect("project");
        let service = ProjectService::open(project.path()).expect("service");
        let registry_path = app_data.path().join("projects.json");
        let runtime = AppRuntime {
            registry_path: registry_path.clone(),
            audit_dir: app_data.path().join("agent-activity"),
            registry: Mutex::new(RegistryData::default()),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        };

        env_registry::update(&registry_path, |registry| {
            registry.projects.push(ProjectRegistration {
                id: service.project_id().to_owned(),
                name: "Broker project".to_owned(),
                display_path: service.root().to_string_lossy().into_owned(),
                root: service.root().to_path_buf(),
                file_labels: BTreeMap::new(),
            });
            Ok(())
        })
        .expect("external registration");

        let projects = runtime.list();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Broker project");
    }

    #[test]
    fn blocked_agent_value_request_enters_review_without_exposing_a_value() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join(".env.local"),
            "CUSTOM_MODE=fake_value\n",
        )
        .expect("synthetic env");
        let audit_dir = app_data.path().join("agent-activity");
        let runtime = AppRuntime {
            registry_path: app_data.path().join("projects.json"),
            audit_dir: audit_dir.clone(),
            registry: Mutex::new(RegistryData::default()),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        };
        let registered = runtime.register(project.path()).expect("register project");
        fs::create_dir_all(&audit_dir).expect("audit directory");
        let event = AgentActivityEvent {
            timestamp_ms: 1,
            project_id: registered.id.clone(),
            actor: "codex".to_owned(),
            category: "value-read".to_owned(),
            operation: "read_allowed_value".to_owned(),
            relative_paths: vec![".env.local".to_owned()],
            variable_names: vec!["CUSTOM_MODE".to_owned()],
            policy_decision: "policy-checked".to_owned(),
            outcome: "blocked".to_owned(),
            result_code: "CODEX_ACCESS_BLOCKED".to_owned(),
        };
        let mut event_bytes = serde_json::to_vec(&event).expect("audit event");
        event_bytes.push(b'\n');
        fs::write(
            audit_dir.join(format!("{}.jsonl", registered.id)),
            event_bytes,
        )
        .expect("audit fixture");

        let projection = runtime.scan(&registered.id).expect("scan project");
        let review = projection
            .classification_review
            .iter()
            .find(|item| item.key == "CUSTOM_MODE")
            .expect("review item");

        assert_eq!(projection.access_review_count, 1);
        assert_eq!(review.access, env_core::CodexAccess::Unclassified);
        assert_eq!(
            review.review_reasons,
            vec![ClassificationReviewReason::AgentAccessRequest]
        );
    }

    #[test]
    fn migrates_legacy_file_labels_to_local_registry_before_removing_them() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = tempfile::tempdir().expect("project");
        let manifest_path = project.path().join(env_core::MANIFEST_FILE_NAME);
        fs::write(
            &manifest_path,
            r#"{
  "version": 1,
  "scan": { "ignoredFiles": [], "ignoredDirectories": [] },
  "variables": {},
  "links": [],
  "fileLabels": { ".env.local": "Local display name" }
}"#,
        )
        .expect("legacy manifest");
        let registry_path = app_data.path().join("projects.json");
        let mut registry = RegistryData {
            projects: vec![ProjectRegistration {
                id: "fake-project".to_owned(),
                name: "Shared project".to_owned(),
                display_path: project.path().to_string_lossy().into_owned(),
                root: project.path().to_path_buf(),
                file_labels: BTreeMap::new(),
            }],
            last_selected_project_id: None,
            team_channels: Vec::new(),
            provider_push_receipts: Vec::new(),
        };

        migrate_legacy_file_labels(&registry_path, &mut registry).expect("migration");

        assert_eq!(
            registry.projects[0]
                .file_labels
                .get(".env.local")
                .map(String::as_str),
            Some("Local display name")
        );
        let local_registry = fs::read_to_string(&registry_path).expect("local registry");
        assert!(local_registry.contains("Local display name"));
        let shared_manifest = fs::read_to_string(&manifest_path).expect("shared manifest");
        assert!(!shared_manifest.contains("fileLabels"));
        assert!(!shared_manifest.contains("Local display name"));
    }

    #[test]
    fn folder_team_channel_path_stays_local_and_missing_mount_is_reported() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = tempfile::tempdir().expect("project");
        let shared = tempfile::tempdir().expect("shared folder");
        let registry_path = app_data.path().join("projects.json");
        let runtime = AppRuntime {
            registry_path: registry_path.clone(),
            audit_dir: app_data.path().join("agent-activity"),
            registry: Mutex::new(RegistryData::default()),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        };
        let registered = runtime.register(project.path()).expect("register project");
        let connected = runtime
            .connect_folder_team_channel(&registered.id, shared.path(), "Synthetic channel")
            .expect("connect channel");

        assert!(connected.readable);
        assert!(connected.publishable);
        let local_registry = fs::read_to_string(&registry_path).expect("local registry");
        assert!(local_registry.contains("teamChannels"));
        let local_registry: serde_json::Value =
            serde_json::from_str(&local_registry).expect("registry json");
        let channel = &local_registry["teamChannels"][0];
        assert_eq!(channel["transport"]["type"], "folder");
        let canonical_shared = shared.path().canonicalize().expect("canonical shared path");
        assert_eq!(
            channel["transport"]["path"],
            canonical_shared.to_string_lossy().as_ref()
        );
        assert!(channel.get("root").is_none());
        assert!(channel.get("channelId").is_none());
        let shared_manifest = fs::read_to_string(project.path().join(env_core::MANIFEST_FILE_NAME))
            .expect("synthetic manifest");
        assert!(!shared_manifest.contains("teamChannel"));

        shared.close().expect("remove mounted folder fixture");
        let channels = runtime
            .list_team_channels(&registered.id)
            .expect("list unavailable channel");
        assert_eq!(channels.len(), 1);
        assert!(!channels[0].readable);
        assert!(!channels[0].publishable);
    }

    #[test]
    fn provider_push_receipt_persists_metadata_without_value_fingerprints() {
        let app_data = tempfile::tempdir().expect("app data");
        let project = tempfile::tempdir().expect("synthetic project");
        let runtime = AppRuntime {
            registry_path: app_data.path().join("projects.json"),
            audit_dir: app_data.path().join("agent-activity"),
            registry: Mutex::new(RegistryData::default()),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        };
        let registered = runtime.register(project.path()).expect("register project");
        runtime
            .record_provider_push(ProviderPushReceipt {
                timestamp_ms: 42,
                project_id: registered.id.clone(),
                provider: "aws-secrets-manager".to_owned(),
                source_file: ".env.local".to_owned(),
                destination: "ap-northeast-2/demo".to_owned(),
                succeeded_keys: vec!["DEMO_TOKEN".to_owned()],
                failed_keys: Vec::new(),
            })
            .expect("record receipt");

        let saved =
            fs::read_to_string(app_data.path().join("projects.json")).expect("read local registry");
        assert!(saved.contains("DEMO_TOKEN"));
        assert!(!saved.contains("valueHash"));
        assert!(!saved.contains("revision"));
        assert!(!saved.contains("fingerprint"));
        assert_eq!(
            runtime
                .provider_push_receipts(&registered.id)
                .expect("list receipts")
                .len(),
            1
        );
    }
}

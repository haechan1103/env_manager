use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use env_core::{
    ClassificationReviewReason, ClassificationSource, DiscoveryOptions, EnvError, EnvErrorCode,
    EnvResult, MigrationPlan, MigrationPreview, MutationSummary, ProjectService, TeamImportPlan,
    TeamImportPreview, TeamImportSummary, TeamImportValueSide, is_env_candidate,
};
use env_team::TeamChannelRegistration;
use notify::{
    Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tempfile::NamedTempFile;

mod agent_activity;
mod team_channels;

pub use team_channels::TeamChannelProjection;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistration {
    pub id: String,
    pub name: String,
    pub display_path: String,
    root: PathBuf,
    #[serde(default)]
    file_labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub display_path: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryData {
    #[serde(default)]
    projects: Vec<ProjectRegistration>,
    #[serde(default)]
    last_selected_project_id: Option<String>,
    #[serde(default)]
    #[serde(deserialize_with = "env_team::deserialize_team_channel_registrations")]
    team_channels: Vec<TeamChannelRegistration>,
    #[serde(default)]
    provider_push_receipts: Vec<ProviderPushReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderPushReceipt {
    pub timestamp_ms: u64,
    pub project_id: String,
    pub provider: String,
    pub source_file: String,
    pub destination: String,
    pub succeeded_keys: Vec<String>,
    pub failed_keys: Vec<String>,
}

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

impl AppRuntime {
    pub fn load(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_data = app.path().app_data_dir()?;
        fs::create_dir_all(&app_data)?;
        let registry_path = app_data.join("projects.json");
        let mut migrate_team_channels = false;
        let mut registry: RegistryData = if registry_path.exists() {
            let bytes = fs::read(&registry_path)?;
            migrate_team_channels = env_team::registry_contains_legacy_team_channels(&bytes);
            serde_json::from_slice(&bytes)?
        } else {
            RegistryData::default()
        };
        migrate_legacy_file_labels(&registry_path, &mut registry)?;
        if migrate_team_channels {
            persist_registry(&registry_path, &registry)?;
        }
        let audit_dir = app_data.join("agent-activity");
        let legacy_audit_dir = std::env::temp_dir().join("env-manager-audit");
        let project_ids = registry
            .projects
            .iter()
            .map(|project| project.id.as_str())
            .collect::<Vec<_>>();
        let _ = agent_activity::migrate_legacy_agent_activity(
            &legacy_audit_dir,
            &audit_dir,
            &project_ids,
        );
        Ok(Self {
            registry_path,
            audit_dir,
            registry: Mutex::new(registry),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
            team_import_plans: Mutex::new(HashMap::new()),
            next_plan_id: AtomicU64::new(1),
        })
    }

    pub fn list(&self) -> Vec<ProjectSummary> {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .projects
            .iter()
            .map(ProjectSummary::from)
            .collect()
    }

    pub fn last_selected_project_id(&self) -> Option<String> {
        let registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry
            .last_selected_project_id
            .as_ref()
            .and_then(|selected| {
                registry
                    .projects
                    .iter()
                    .any(|project| project.id == *selected)
                    .then(|| selected.clone())
            })
    }

    pub fn remember_selected_project(&self, project_id: Option<&str>) -> EnvResult<()> {
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(project_id) = project_id
            && !registry
                .projects
                .iter()
                .any(|project| project.id == project_id)
        {
            return Err(EnvError::unregistered_project(project_id));
        }
        registry.last_selected_project_id = project_id.map(str::to_owned);
        self.persist(&registry)
    }

    pub fn register(&self, root: &Path) -> EnvResult<ProjectSummary> {
        let service = ProjectService::open(root)?;
        let name = service.root().file_name().map_or_else(
            || "Project".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let registration = ProjectRegistration {
            id: service.project_id().to_owned(),
            name,
            display_path: service.root().to_string_lossy().into_owned(),
            root: service.root().to_path_buf(),
            file_labels: BTreeMap::new(),
        };
        {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(existing) = registry
                .projects
                .iter_mut()
                .find(|project| project.id == registration.id)
            {
                existing.display_path = registration.display_path.clone();
                existing.root = registration.root.clone();
                let migrated_labels = migrate_registration_labels(existing, &service)?;
                let summary = ProjectSummary::from(&*existing);
                let file_labels = existing.file_labels.clone();
                self.persist(&registry)?;
                if migrated_labels {
                    env_core::ManifestStore::for_root(service.root()).take_legacy_file_labels()?;
                }
                service.initialize_with_file_labels(&file_labels)?;
                return Ok(summary);
            } else {
                registry.projects.push(registration.clone());
                registry.projects.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                });
            }
            self.persist(&registry)?;
        }
        {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(project) = registry
                .projects
                .iter_mut()
                .find(|project| project.id == registration.id)
            {
                let migrated_labels = migrate_registration_labels(project, &service)?;
                let file_labels = project.file_labels.clone();
                self.persist(&registry)?;
                if migrated_labels {
                    env_core::ManifestStore::for_root(service.root()).take_legacy_file_labels()?;
                }
                service.initialize_with_file_labels(&file_labels)?;
            }
        }
        Ok(ProjectSummary::from(&registration))
    }

    pub fn rename_project(&self, project_id: &str, name: &str) -> EnvResult<ProjectSummary> {
        env_core::validate_display_name(name)?;
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = registry
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .ok_or_else(|| EnvError::unregistered_project(project_id))?;
        project.name = name.trim().to_owned();
        let summary = ProjectSummary::from(&*project);
        registry.projects.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        self.persist(&registry)?;
        Ok(summary)
    }

    pub fn remove(&self, project_id: &str) -> EnvResult<()> {
        {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let before = registry.projects.len();
            registry.projects.retain(|project| project.id != project_id);
            if before == registry.projects.len() {
                return Err(EnvError::unregistered_project(project_id));
            }
            if registry.last_selected_project_id.as_deref() == Some(project_id) {
                registry.last_selected_project_id =
                    registry.projects.first().map(|project| project.id.clone());
            }
            registry
                .team_channels
                .retain(|channel| channel.project_id != project_id);
            registry
                .provider_push_receipts
                .retain(|receipt| receipt.project_id != project_id);
            self.persist(&registry)?;
        }
        self.watchers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(project_id);
        Ok(())
    }

    pub fn service(&self, project_id: &str) -> EnvResult<ProjectService> {
        let root = self.root(project_id)?;
        ProjectService::open(root)
    }

    pub fn scan(&self, project_id: &str) -> EnvResult<env_core::ProjectProjection> {
        let (root, file_labels) = {
            let registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let project = registry
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .ok_or_else(|| EnvError::unregistered_project(project_id))?;
            (project.root.clone(), project.file_labels.clone())
        };
        let mut projection =
            ProjectService::open(root)?.initialize_with_file_labels(&file_labels)?;
        if let Ok(activity) = self.agent_activity(project_id) {
            let requested_keys = activity
                .into_iter()
                .filter(|event| event.category == "value-read" && event.outcome == "blocked")
                .flat_map(|event| event.variable_names)
                .collect::<BTreeSet<_>>();
            for item in &mut projection.classification_review {
                if item.classified_by == ClassificationSource::Heuristic
                    && requested_keys.contains(&item.key)
                    && !item
                        .review_reasons
                        .contains(&ClassificationReviewReason::AgentAccessRequest)
                {
                    item.review_reasons
                        .push(ClassificationReviewReason::AgentAccessRequest);
                }
            }
            projection.access_review_count = projection
                .classification_review
                .iter()
                .filter(|item| !item.review_reasons.is_empty())
                .count();
        }
        Ok(projection)
    }

    pub fn rename_file(&self, project_id: &str, file: &str, name: &str) -> EnvResult<()> {
        env_core::validate_display_name(name)?;
        let service = self.service(project_id)?;
        let path = service.validate_file_for_display_name(file)?;
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let project = registry
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .ok_or_else(|| EnvError::unregistered_project(project_id))?;
        project.file_labels.insert(path, name.trim().to_owned());
        self.persist(&registry)
    }

    pub fn agent_activity(&self, project_id: &str) -> EnvResult<Vec<AgentActivityEvent>> {
        // Resolve through the registration first so arbitrary file names cannot be requested.
        let service = self.service(project_id)?;
        Ok(agent_activity::load_agent_activity(
            &self.audit_dir,
            &std::env::temp_dir().join("env-manager-audit"),
            service.project_id(),
        ))
    }

    pub fn provider_push_receipts(&self, project_id: &str) -> EnvResult<Vec<ProviderPushReceipt>> {
        let _ = self.root(project_id)?;
        let registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(registry
            .provider_push_receipts
            .iter()
            .filter(|receipt| receipt.project_id == project_id)
            .take(100)
            .cloned()
            .collect())
    }

    pub fn record_provider_push(&self, receipt: ProviderPushReceipt) -> EnvResult<()> {
        let _ = self.root(&receipt.project_id)?;
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.provider_push_receipts.insert(0, receipt);
        registry.provider_push_receipts.truncate(500);
        self.persist(&registry)
    }

    pub fn plan_migration(
        &self,
        project_id: &str,
        file: &str,
    ) -> EnvResult<MigrationPlanProjection> {
        let plan = self.service(project_id)?.plan_migration(file)?;
        let plan_id = format!(
            "migration-{}-{}",
            project_id,
            self.next_plan_id.fetch_add(1, Ordering::Relaxed)
        );
        let preview = plan.preview.clone();
        self.migration_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                plan_id.clone(),
                StoredMigration {
                    project_id: project_id.to_owned(),
                    expires_at: Instant::now() + Duration::from_secs(300),
                    plan,
                },
            );
        Ok(MigrationPlanProjection {
            plan_id,
            expires_in_seconds: 300,
            preview,
        })
    }

    pub fn apply_migration(&self, project_id: &str, plan_id: &str) -> EnvResult<MutationSummary> {
        let stored = self
            .migration_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(plan_id)
            .ok_or_else(|| {
                EnvError::new(
                    env_core::EnvErrorCode::PlanExpired,
                    "정리 계획이 만료되었습니다.",
                )
            })?;
        if stored.project_id != project_id || stored.expires_at < Instant::now() {
            return Err(EnvError::new(
                env_core::EnvErrorCode::PlanExpired,
                "정리 계획이 만료되었습니다.",
            ));
        }
        self.service(project_id)?.apply_migration(stored.plan)
    }

    pub fn plan_team_import(
        &self,
        project_id: &str,
        package: &Path,
        passphrase: age::secrecy::SecretString,
    ) -> EnvResult<TeamImportPlanProjection> {
        let service = self.service(project_id)?;
        let manifest = env_core::ManifestStore::for_root(service.root()).load()?;
        let plan =
            env_core::plan_encrypted_team_import(service.root(), &manifest, package, passphrase)?;
        self.store_team_import_plan(project_id, plan)
    }

    fn store_team_import_plan(
        &self,
        project_id: &str,
        plan: TeamImportPlan,
    ) -> EnvResult<TeamImportPlanProjection> {
        let preview = plan.preview().clone();
        let plan_id = format!(
            "team-import-{}-{}",
            project_id,
            self.next_plan_id.fetch_add(1, Ordering::Relaxed)
        );
        self.team_import_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                plan_id.clone(),
                StoredTeamImport {
                    project_id: project_id.to_owned(),
                    expires_at: Instant::now() + Duration::from_secs(300),
                    plan,
                },
            );
        Ok(TeamImportPlanProjection {
            plan_id,
            expires_in_seconds: 300,
            preview,
        })
    }

    pub fn apply_team_import(
        &self,
        project_id: &str,
        plan_id: &str,
        shared_conflicts: &[String],
    ) -> EnvResult<TeamImportSummary> {
        let stored = self
            .team_import_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(plan_id)
            .ok_or_else(team_import_plan_expired)?;
        if stored.project_id != project_id || stored.expires_at < Instant::now() {
            return Err(team_import_plan_expired());
        }
        stored.plan.apply(shared_conflicts)
    }

    pub fn remap_team_import_file(
        &self,
        project_id: &str,
        plan_id: &str,
        source_file: &str,
        target_file: &str,
    ) -> EnvResult<TeamImportPreview> {
        let mut plans = self
            .team_import_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stored = plans
            .get_mut(plan_id)
            .ok_or_else(team_import_plan_expired)?;
        if stored.project_id != project_id || stored.expires_at < Instant::now() {
            plans.remove(plan_id);
            return Err(team_import_plan_expired());
        }
        stored.plan.remap_file(source_file, target_file)
    }

    pub fn reveal_team_import_conflict(
        &self,
        project_id: &str,
        plan_id: &str,
        occurrence_id: &str,
        side: TeamImportValueSide,
    ) -> EnvResult<String> {
        let plans = self
            .team_import_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stored = plans.get(plan_id).ok_or_else(team_import_plan_expired)?;
        if stored.project_id != project_id || stored.expires_at < Instant::now() {
            return Err(team_import_plan_expired());
        }
        let value = stored.plan.reveal_conflict(occurrence_id, side)?;
        Ok(value.to_string())
    }

    pub fn discard_team_import(&self, project_id: &str, plan_id: &str) {
        let mut plans = self
            .team_import_plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if plans
            .get(plan_id)
            .is_some_and(|stored| stored.project_id == project_id)
        {
            plans.remove(plan_id);
        }
    }

    pub fn start_watching(
        &self,
        app: &AppHandle,
        project_id: &str,
        managed_relative_paths: &[String],
    ) -> EnvResult<()> {
        let root = self.root(project_id)?;
        let mut absolute_paths = BTreeSet::new();
        for relative in managed_relative_paths {
            let absolute = root
                .join(relative)
                .canonicalize()
                .map_err(|error| EnvError::io(Path::new(relative), error))?;
            if absolute.starts_with(&root) {
                absolute_paths.insert(absolute);
            }
        }

        let (sender, receiver) = mpsc::channel::<PathBuf>();
        let managed_paths_for_events = absolute_paths.clone();
        let root_for_filter = root.clone();
        let ignored_directories = DiscoveryOptions::default().ignored_directories;
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        if should_rescan_for_event(
                            &root_for_filter,
                            &path,
                            &event.kind,
                            &managed_paths_for_events,
                            &ignored_directories,
                        ) {
                            let _ = sender.send(path);
                        }
                    }
                }
            },
            Config::default(),
        )
        .map_err(|_| EnvError::invalid("파일 감시기를 시작하지 못했습니다."))?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|_| EnvError::invalid("env 파일을 감시하지 못했습니다."))?;

        let app = app.clone();
        let root_for_events = root.clone();
        let project_id_for_events = project_id.to_owned();
        std::thread::spawn(move || {
            while let Ok(first) = receiver.recv() {
                let mut changed = BTreeSet::from([first]);
                while let Ok(next) = receiver.recv_timeout(Duration::from_millis(400)) {
                    changed.insert(next);
                }
                let paths = changed
                    .into_iter()
                    .filter_map(|path| {
                        path.strip_prefix(&root_for_events)
                            .ok()
                            .map(to_relative_string)
                    })
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    let _ = app.emit(
                        "managed-files-changed",
                        ManagedFilesChanged {
                            project_id: project_id_for_events.clone(),
                            paths,
                        },
                    );
                }
            }
        });

        self.watchers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(project_id.to_owned(), watcher);
        Ok(())
    }

    fn root(&self, project_id: &str) -> EnvResult<PathBuf> {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .projects
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.root.clone())
            .ok_or_else(|| EnvError::unregistered_project(project_id))
    }

    fn persist(&self, registry: &RegistryData) -> EnvResult<()> {
        persist_registry(&self.registry_path, registry)
    }
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
    let parent = path
        .parent()
        .ok_or_else(|| EnvError::invalid("앱 상태 경로가 올바르지 않습니다."))?;
    let mut staged = NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), registry)
        .map_err(EnvError::serialization)?;
    staged
        .write_all(b"\n")
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .persist(path)
        .map_err(|error| EnvError::io(path, error.error))?;
    Ok(())
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
        assert!(local_registry.contains(&shared.path().to_string_lossy().to_string()));
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
        assert!(!shared_manifest.contains(&shared.path().to_string_lossy().to_string()));

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

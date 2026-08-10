use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use env_core::{
    EnvError, EnvResult, MigrationPlan, MigrationPreview, MutationSummary, ProjectService,
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistration {
    pub id: String,
    pub name: String,
    pub display_path: String,
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub display_path: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryData {
    projects: Vec<ProjectRegistration>,
}

pub struct AppRuntime {
    registry_path: PathBuf,
    audit_dir: PathBuf,
    registry: Mutex<RegistryData>,
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    migration_plans: Mutex<HashMap<String, StoredMigration>>,
    next_plan_id: AtomicU64,
}

struct StoredMigration {
    project_id: String,
    expires_at: Instant,
    plan: MigrationPlan,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlanProjection {
    pub plan_id: String,
    pub expires_in_seconds: u64,
    pub preview: MigrationPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let registry = if registry_path.exists() {
            let bytes = fs::read(&registry_path)?;
            serde_json::from_slice(&bytes)?
        } else {
            RegistryData::default()
        };
        Ok(Self {
            registry_path,
            audit_dir: app_data.join("agent-activity"),
            registry: Mutex::new(registry),
            watchers: Mutex::new(HashMap::new()),
            migration_plans: Mutex::new(HashMap::new()),
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
                *existing = registration.clone();
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
        service.initialize()?;
        Ok(ProjectSummary::from(&registration))
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

    pub fn agent_activity(&self, project_id: &str) -> EnvResult<Vec<AgentActivityEvent>> {
        // Resolve through the registration first so arbitrary file names cannot be requested.
        let service = self.service(project_id)?;
        let path = self
            .audit_dir
            .join(format!("{}.jsonl", service.project_id()));
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(EnvError::invalid("AI 활동 기록을 읽지 못했습니다.")),
        };
        let start = bytes.len().saturating_sub(2 * 1024 * 1024);
        let slice = if start == 0 {
            &bytes[..]
        } else {
            let offset = bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| start + offset + 1);
            &bytes[offset..]
        };
        let mut events = slice
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_slice::<AgentActivityEvent>(line).ok())
            .filter(|event| event.project_id == project_id)
            .collect::<Vec<_>>();
        events.sort_by_key(|event| std::cmp::Reverse(event.timestamp_ms));
        events.truncate(200);
        Ok(events)
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
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        let _ = sender.send(path);
                    }
                }
            },
            Config::default(),
        )
        .map_err(|_| EnvError::invalid("파일 감시기를 시작하지 못했습니다."))?;

        let parent_directories = absolute_paths
            .iter()
            .filter_map(|path| path.parent().map(Path::to_path_buf))
            .collect::<BTreeSet<_>>();
        for path in &parent_directories {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|_| EnvError::invalid("env 파일을 감시하지 못했습니다."))?;
        }

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
                    .filter(|path| managed_paths_for_events.contains(path))
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
        let parent = self
            .registry_path
            .parent()
            .ok_or_else(|| EnvError::invalid("앱 상태 경로가 올바르지 않습니다."))?;
        let mut staged =
            NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
        serde_json::to_writer_pretty(staged.as_file_mut(), registry)
            .map_err(EnvError::serialization)?;
        staged
            .write_all(b"\n")
            .map_err(|error| EnvError::io(&self.registry_path, error))?;
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|error| EnvError::io(&self.registry_path, error))?;
        staged
            .persist(&self.registry_path)
            .map_err(|error| EnvError::io(&self.registry_path, error.error))?;
        Ok(())
    }
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

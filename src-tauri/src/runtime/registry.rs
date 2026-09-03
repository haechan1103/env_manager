use super::*;

impl AppRuntime {
    pub fn load(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_data = app.path().app_data_dir()?;
        fs::create_dir_all(&app_data)?;
        let registry_path = app_data.join("projects.json");
        let migrate_team_channels = registry_path.exists()
            && env_team::registry_contains_legacy_team_channels(&fs::read(&registry_path)?);
        let mut registry = env_registry::read(&registry_path)?;
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
        self.refresh_registry_best_effort();
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .projects
            .iter()
            .map(ProjectSummary::from)
            .collect()
    }

    pub fn last_selected_project_id(&self) -> Option<String> {
        self.refresh_registry_best_effort();
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
        self.update_registry(|registry| {
            if let Some(project_id) = project_id
                && !registry
                    .projects
                    .iter()
                    .any(|project| project.id == project_id)
            {
                return Err(EnvError::unregistered_project(project_id));
            }
            registry.last_selected_project_id = project_id.map(str::to_owned);
            Ok(())
        })
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
        let (summary, file_labels, migrated_labels) = self.update_registry(|registry| {
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
                Ok((summary, file_labels, migrated_labels))
            } else {
                let mut registration = registration.clone();
                let migrated_labels = migrate_registration_labels(&mut registration, &service)?;
                let file_labels = registration.file_labels.clone();
                let summary = ProjectSummary::from(&registration);
                registry.projects.push(registration);
                registry.projects.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                });
                Ok((summary, file_labels, migrated_labels))
            }
        })?;
        if migrated_labels {
            env_core::ManifestStore::for_root(service.root()).take_legacy_file_labels()?;
        }
        service.initialize_with_file_labels(&file_labels)?;
        Ok(summary)
    }

    pub fn rename_project(&self, project_id: &str, name: &str) -> EnvResult<ProjectSummary> {
        env_core::validate_display_name(name)?;
        self.update_registry(|registry| {
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
            Ok(summary)
        })
    }

    pub fn remove(&self, project_id: &str) -> EnvResult<()> {
        self.update_registry(|registry| {
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
            Ok(())
        })?;
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

    pub(super) fn root(&self, project_id: &str) -> EnvResult<PathBuf> {
        self.refresh_registry()?;
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .projects
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.root.clone())
            .ok_or_else(|| EnvError::unregistered_project(project_id))
    }

    pub(super) fn refresh_registry(&self) -> EnvResult<()> {
        let latest = env_registry::read(&self.registry_path)?;
        *self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = latest;
        Ok(())
    }

    pub(super) fn refresh_registry_best_effort(&self) {
        let _ = self.refresh_registry();
    }

    pub(super) fn update_registry<R>(
        &self,
        operation: impl FnOnce(&mut RegistryData) -> EnvResult<R>,
    ) -> EnvResult<R> {
        let (latest, result) = env_registry::update(&self.registry_path, operation)?;
        *self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = latest;
        Ok(result)
    }
}

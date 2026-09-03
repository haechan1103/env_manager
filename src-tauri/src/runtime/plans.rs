use super::*;

impl AppRuntime {
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

    pub(super) fn store_team_import_plan(
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
}

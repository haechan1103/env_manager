use super::*;

impl ProjectService {
    pub fn plan_migration(&self, file: &str) -> EnvResult<crate::MigrationPlan> {
        let relative = PathBuf::from(file);
        let loaded = self.load_document(&relative)?;
        crate::MigrationPlan::build(file.to_owned(), &loaded.document, loaded.revision)
    }

    pub fn apply_migration(&self, plan: crate::MigrationPlan) -> EnvResult<MutationSummary> {
        let file = plan.preview.file.clone();
        plan.apply(&self.root)?;
        Ok(MutationSummary {
            affected_files: vec![file],
            keys: Vec::new(),
        })
    }

    pub fn apply_gitignore_guard(&self) -> EnvResult<crate::GitignoreUpdateSummary> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        let files = self.discover(&manifest)?;
        crate::apply_gitignore_guard(&self.root, &files)
    }

    pub fn export_env_files(
        &self,
        destination: &Path,
        passphrase: Option<String>,
        selection: Option<&[crate::ExportOccurrence]>,
    ) -> EnvResult<crate::ExportSummary> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        crate::export_project_env(
            &self.root,
            &manifest,
            destination,
            passphrase.map(age::secrecy::SecretString::from),
            selection,
        )
    }
}

use super::*;

impl ProjectService {
    pub(super) fn commit_one(
        &self,
        relative: PathBuf,
        revision: FileRevision,
        proposed: Vec<u8>,
    ) -> EnvResult<()> {
        TransactionPlan::new(vec![PlannedFileChange {
            relative_path: relative,
            expected_revision: revision,
            proposed_bytes: proposed,
        }])
        .commit(&self.root)
    }

    pub(super) fn ensure_policy(&self, key: &str) -> EnvResult<()> {
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        if manifest.variables.contains_key(key) {
            return Ok(());
        }
        manifest.variables.insert(
            key.to_owned(),
            VariablePolicy {
                codex_access: default_access(key),
                classified_by: ClassificationSource::Heuristic,
            },
        );
        store.save(&manifest)
    }

    pub(super) fn commit_files_then_manifest(
        &self,
        changes: Vec<PlannedFileChange>,
        store: &ManifestStore,
        manifest: &Manifest,
    ) -> EnvResult<()> {
        let originals = changes
            .iter()
            .map(|change| {
                let target = safe_existing_target(&self.root, &change.relative_path)?;
                let bytes = fs::read(&target)
                    .map_err(|error| EnvError::io(&change.relative_path, error))?;
                Ok((change.relative_path.clone(), bytes))
            })
            .collect::<EnvResult<Vec<_>>>()?;
        TransactionPlan::new(changes).commit(&self.root)?;
        if let Err(manifest_error) = store.save(manifest) {
            let rollback = originals
                .into_iter()
                .map(|(relative_path, original)| {
                    let current = fs::read(self.root.join(&relative_path))
                        .map_err(|error| EnvError::io(&relative_path, error))?;
                    Ok(PlannedFileChange {
                        relative_path,
                        expected_revision: FileRevision::from_bytes(&current),
                        proposed_bytes: original,
                    })
                })
                .collect::<EnvResult<Vec<_>>>()
                .and_then(|changes| TransactionPlan::new(changes).commit(&self.root));
            return match rollback {
                Ok(()) => Err(manifest_error),
                Err(_) => Err(EnvError::transaction(
                    "연결 저장 중 manifest와 env 파일 복구가 완전하지 않을 수 있습니다.",
                )),
            };
        }
        Ok(())
    }
}

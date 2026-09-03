use super::*;

impl ProjectService {
    pub fn set_codex_access(&self, key: &str, access: CodexAccess) -> EnvResult<()> {
        self.set_codex_access_by(key, access, ClassificationSource::User)
    }

    pub fn set_codex_access_by(
        &self,
        key: &str,
        access: CodexAccess,
        classified_by: ClassificationSource,
    ) -> EnvResult<()> {
        validate_key(key)?;
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        manifest.variables.insert(
            key.to_owned(),
            VariablePolicy {
                codex_access: access,
                classified_by,
            },
        );
        store.save(&manifest)
    }

    pub fn set_codex_access_batch(&self, keys: &[String], access: CodexAccess) -> EnvResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        for key in keys {
            validate_key(key)?;
            manifest.variables.insert(
                key.clone(),
                VariablePolicy {
                    codex_access: access,
                    classified_by: ClassificationSource::User,
                },
            );
        }
        store.save(&manifest)
    }

    pub fn validate_file_for_display_name(&self, file: &str) -> EnvResult<String> {
        let relative = PathBuf::from(file);
        let _ = safe_existing_target(&self.root, &relative)?;
        if !relative
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_env_candidate)
        {
            return Err(EnvError::invalid(
                "관리 중인 env 파일만 이름을 지정할 수 있습니다.",
            ));
        }
        Ok(to_manifest_path(&relative))
    }

    pub fn codex_access(&self, key: &str) -> EnvResult<CodexAccess> {
        validate_key(key)?;
        Ok(ManifestStore::for_root(&self.root).load()?.access_for(key))
    }

    pub fn read_value(&self, file: &str, key: &str) -> EnvResult<String> {
        let loaded = self.load_document(Path::new(file))?;
        loaded.document.decoded_value(key)
    }

    pub fn read_allowed_value(&self, file: &str, key: &str) -> EnvResult<String> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        if manifest.access_for(key) != CodexAccess::ReadWrite {
            return Err(EnvError::access_blocked(key));
        }
        self.read_value(file, key)
    }

    pub fn provider_values(&self, file: &str, keys: &[String]) -> EnvResult<Vec<ProviderValue>> {
        if keys.is_empty() || keys.len() > 100 {
            return Err(EnvError::invalid(
                "한 번에 1개 이상 100개 이하의 변수를 선택해주세요.",
            ));
        }
        let unique = keys.iter().collect::<BTreeSet<_>>();
        if unique.len() != keys.len() {
            return Err(EnvError::invalid("같은 변수를 중복 선택할 수 없습니다."));
        }

        let manifest = ManifestStore::for_root(&self.root).load()?;
        let relative = PathBuf::from(file);
        if !self
            .discover(&manifest)?
            .iter()
            .any(|path| path == &relative)
        {
            return Err(EnvError::invalid(
                "관리 중인 env 파일만 전송할 수 있습니다.",
            ));
        }
        let loaded = self.load_document(&relative)?;
        keys.iter()
            .map(|key| {
                validate_key(key)?;
                let value = loaded.document.decoded_value(key)?;
                if value.is_empty() {
                    return Err(EnvError::invalid(format!(
                        "값이 비어 있는 {key} 변수는 전송할 수 없습니다."
                    )));
                }
                Ok(ProviderValue::new(key.clone(), value))
            })
            .collect()
    }
}

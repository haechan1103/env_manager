use super::*;

impl ProjectService {
    pub fn create_link(&self, request: LinkRequest) -> EnvResult<MutationSummary> {
        validate_key(&request.key)?;
        let unique = request.files.iter().cloned().collect::<BTreeSet<_>>();
        if unique.len() < 2 {
            return Err(EnvError::invalid("연결은 두 개 이상의 파일이 필요합니다."));
        }
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        if manifest.links.iter().any(|link| {
            link.key == request.key
                && link
                    .members
                    .iter()
                    .any(|member| unique.contains(&member.file))
        }) {
            return Err(EnvError::invalid(
                "선택한 occurrence 중 하나가 이미 연결되어 있습니다.",
            ));
        }

        let mut loaded_files = BTreeMap::new();
        let mut distinct_values = BTreeSet::new();
        for file in &unique {
            let relative = PathBuf::from(file);
            let loaded = self.load_document(&relative)?;
            let value = loaded
                .document
                .decoded_value(&request.key)
                .map_err(|_| EnvError::link_member_missing(&request.key, &relative))?;
            if !value.is_empty() {
                distinct_values.insert(value);
            }
            loaded_files.insert(file.clone(), loaded);
        }

        let source_value = if let Some(source_file) = &request.source_file {
            loaded_files
                .get(source_file)
                .ok_or_else(|| EnvError::invalid("선택한 원본 파일이 연결 대상에 없습니다."))?
                .document
                .decoded_value(&request.key)?
        } else if distinct_values.len() <= 1 {
            distinct_values.into_iter().next().unwrap_or_default()
        } else {
            return Err(EnvError::link_conflict(&request.key));
        };

        let mut changes = Vec::new();
        for (file, loaded) in &loaded_files {
            changes.push(PlannedFileChange {
                relative_path: PathBuf::from(file),
                expected_revision: loaded.revision.clone(),
                proposed_bytes: loaded.document.replace_value(&request.key, &source_value)?,
            });
        }

        let link_id_seed = format!(
            "{}:{}:{}",
            self.project_id,
            request.key,
            unique.iter().cloned().collect::<Vec<_>>().join("|")
        );
        manifest.links.push(LinkGroup {
            id: format!(
                "{}-{}",
                request.key.to_ascii_lowercase().replace('_', "-"),
                &blake3::hash(link_id_seed.as_bytes()).to_hex()[..8]
            ),
            key: request.key.clone(),
            members: unique
                .iter()
                .map(|file| LinkMember { file: file.clone() })
                .collect(),
        });
        manifest.validate()?;

        self.commit_files_then_manifest(changes, &store, &manifest)?;
        Ok(MutationSummary {
            affected_files: unique.into_iter().collect(),
            keys: vec![request.key],
        })
    }

    pub fn detach_link_member(&self, link_id: &str, file: &str) -> EnvResult<()> {
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        let index = manifest
            .links
            .iter()
            .position(|link| link.id == link_id)
            .ok_or_else(|| EnvError::invalid("연결을 찾지 못했습니다."))?;
        manifest.links[index]
            .members
            .retain(|member| member.file != file);
        if manifest.links[index].members.len() < 2 {
            manifest.links.remove(index);
        }
        store.save(&manifest)
    }
}

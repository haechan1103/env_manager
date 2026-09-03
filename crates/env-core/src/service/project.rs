use super::*;

impl ProjectService {
    pub fn open(root: impl AsRef<Path>) -> EnvResult<Self> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|error| EnvError::io(root.as_ref(), error))?;
        if !root.is_dir() {
            return Err(EnvError::invalid("등록할 경로가 디렉터리가 아닙니다."));
        }
        let project_id = blake3::hash(root.to_string_lossy().as_bytes())
            .to_hex()
            .chars()
            .take(16)
            .collect();
        Ok(Self { root, project_id })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn initialize(&self) -> EnvResult<ProjectProjection> {
        self.initialize_with_file_labels(&BTreeMap::new())
    }

    pub fn initialize_with_file_labels(
        &self,
        file_labels: &BTreeMap<String, String>,
    ) -> EnvResult<ProjectProjection> {
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        let files = self.discover(&manifest)?;
        let mut changed = false;

        for relative in &files {
            let loaded = self.load_document(relative)?;
            for assignment in loaded.document.assignments() {
                if let Some(policy) = manifest.variables.get_mut(assignment.key) {
                    if policy.classified_by == ClassificationSource::Heuristic
                        && policy.codex_access == CodexAccess::ReadWrite
                    {
                        policy.codex_access = CodexAccess::Unclassified;
                        changed = true;
                    }
                    continue;
                }
                manifest.variables.insert(
                    assignment.key.to_owned(),
                    VariablePolicy {
                        codex_access: default_access(assignment.key),
                        classified_by: ClassificationSource::Heuristic,
                    },
                );
                changed = true;
            }
        }

        if changed || !self.root.join(MANIFEST_FILE_NAME).exists() {
            store.save(&manifest)?;
        }
        self.scan_with_manifest(&manifest, file_labels)
    }

    pub fn scan(&self) -> EnvResult<ProjectProjection> {
        self.initialize()
    }

    pub(super) fn scan_with_manifest(
        &self,
        manifest: &Manifest,
        file_labels: &BTreeMap<String, String>,
    ) -> EnvResult<ProjectProjection> {
        let files = self.discover(manifest)?;
        let mut projections = Vec::with_capacity(files.len());
        let mut unclassified = BTreeSet::new();
        let mut issue_count = 0;

        for relative in files {
            let loaded = self.load_document(&relative)?;
            let path = to_manifest_path(&relative);
            let projection = project_file(
                &path,
                &loaded.document,
                manifest,
                file_labels,
                &mut unclassified,
            );
            issue_count += projection.warnings.len();
            projections.push(projection);
        }

        let git_safety = crate::inspect_git_safety(&self.root, &files_for_git_safety(&projections));
        let mut review_by_key = BTreeMap::<String, (BTreeSet<String>, bool)>::new();
        for file in &projections {
            for variable in file.groups.iter().flat_map(|group| &group.variables) {
                let entry = review_by_key
                    .entry(variable.key.clone())
                    .or_insert_with(|| (BTreeSet::new(), false));
                entry.0.insert(file.path.clone());
                entry.1 |= variable.client_exposure.is_some();
            }
        }
        let classification_review = review_by_key
            .into_iter()
            .map(|(key, (files, client_exposed))| {
                let policy = manifest.variables.get(&key);
                let classified_by =
                    policy.map_or(ClassificationSource::Heuristic, |item| item.classified_by);
                let review_reasons =
                    if client_exposed && classified_by == ClassificationSource::Heuristic {
                        vec![ClassificationReviewReason::ClientExposureConflict]
                    } else {
                        Vec::new()
                    };
                ClassificationReviewProjection {
                    suggestion: suggest_access(&key),
                    access: policy.map_or(CodexAccess::Unclassified, |item| item.codex_access),
                    classified_by,
                    key,
                    files: files.into_iter().collect(),
                    client_exposed,
                    review_reasons,
                }
            })
            .collect::<Vec<_>>();
        let access_review_count = classification_review
            .iter()
            .filter(|item| !item.review_reasons.is_empty())
            .count();
        let client_exposure_count = projections
            .iter()
            .flat_map(|file| file.groups.iter())
            .flat_map(|group| group.variables.iter())
            .filter(|variable| variable.client_exposure.is_some())
            .count();
        Ok(ProjectProjection {
            project_id: self.project_id.clone(),
            name: self.root.file_name().map_or_else(
                || "Project".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            ),
            files: projections,
            unclassified_count: unclassified.len(),
            issue_count,
            git_safety,
            classification_review,
            access_review_count,
            client_exposure_count,
        })
    }

    pub(super) fn discover(&self, manifest: &Manifest) -> EnvResult<Vec<PathBuf>> {
        let mut options = DiscoveryOptions::default();
        options
            .ignored_files
            .extend(manifest.scan.ignored_files.iter().cloned());
        options
            .ignored_directories
            .extend(manifest.scan.ignored_directories.iter().cloned());
        discover_env_files(&self.root, &options)
    }

    pub(super) fn load_document(&self, relative: &Path) -> EnvResult<LoadedDocument> {
        let target = safe_existing_target(&self.root, relative)?;
        let bytes = fs::read(&target).map_err(|error| EnvError::io(relative, error))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(EnvError::file_too_large(relative));
        }
        let revision = FileRevision::from_bytes(&bytes);
        let document = Document::parse(bytes, relative)?;
        Ok(LoadedDocument { document, revision })
    }

    pub(super) fn load_managed_document(&self, file: &str) -> EnvResult<LoadedDocument> {
        let relative = PathBuf::from(file);
        let manifest = ManifestStore::for_root(&self.root).load()?;
        if !self
            .discover(&manifest)?
            .iter()
            .any(|path| path == &relative)
        {
            return Err(EnvError::invalid(
                "관리 중인 env 파일의 변수만 프로젝트 간 복사할 수 있습니다.",
            ));
        }
        self.load_document(&relative)
    }
}

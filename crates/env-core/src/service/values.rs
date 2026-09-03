use super::*;

impl ProjectService {
    pub fn create_env_file(&self, request: CreateEnvFileRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let manifest = ManifestStore::for_root(&self.root).load()?;
        let mut options = DiscoveryOptions::default();
        options
            .ignored_files
            .extend(manifest.scan.ignored_files.iter().cloned());
        options
            .ignored_directories
            .extend(manifest.scan.ignored_directories.iter().cloned());
        let target = safe_new_env_target(&self.root, &relative, &options)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| EnvError::io(&relative, error))?;
        if let Err(error) = file.sync_all() {
            drop(file);
            let _ = fs::remove_file(&target);
            return Err(EnvError::io(&relative, error));
        }
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: Vec::new(),
        })
    }

    pub fn save_value(&self, request: SaveValueRequest) -> EnvResult<MutationSummary> {
        self.save_value_inner(&request.file, &request.key, &request.new_value)
    }

    /// Prepares a value-free, short-lived write guard for a separate stdin
    /// producer process. Every linked member and the link manifest are bound to
    /// the plan without persisting file-content hashes.
    pub fn prepare_opaque_value_write(
        &self,
        file: &str,
        key: &str,
    ) -> EnvResult<PreparedOpaqueValueWrite> {
        validate_key(key)?;
        let targets = self.checked_value_write_targets(file, key)?;
        let file_states = targets
            .iter()
            .map(|target| self.opaque_file_state(target))
            .collect::<EnvResult<Vec<_>>>()?;
        let manifest_state = self.opaque_file_state(MANIFEST_FILE_NAME)?;
        Ok(PreparedOpaqueValueWrite {
            project_id: self.project_id.clone(),
            file: file.to_owned(),
            key: key.to_owned(),
            affected_files: targets,
            file_states,
            manifest_state,
        })
    }

    /// Applies a previously prepared opaque write through the normal linked,
    /// lossless, optimistic-concurrency transaction.
    pub fn apply_prepared_opaque_value(
        &self,
        prepared: &PreparedOpaqueValueWrite,
        new_value: &str,
    ) -> EnvResult<MutationSummary> {
        if prepared.project_id != self.project_id {
            return Err(EnvError::unregistered_project(&prepared.project_id));
        }
        validate_key(&prepared.key)?;
        let current_targets = self.checked_value_write_targets(&prepared.file, &prepared.key)?;
        if current_targets != prepared.affected_files
            || self.opaque_file_state(MANIFEST_FILE_NAME)? != prepared.manifest_state
        {
            return Err(EnvError::changed_externally(Path::new(MANIFEST_FILE_NAME)));
        }
        for expected in &prepared.file_states {
            if self.opaque_file_state(&expected.relative_path)? != *expected {
                return Err(EnvError::changed_externally(Path::new(
                    &expected.relative_path,
                )));
            }
        }
        self.save_value_inner(&prepared.file, &prepared.key, new_value)
    }

    pub(super) fn save_value_inner(
        &self,
        file: &str,
        key: &str,
        new_value: &str,
    ) -> EnvResult<MutationSummary> {
        validate_key(key)?;
        let targets = self.checked_value_write_targets(file, key)?;
        let mut changes = Vec::with_capacity(targets.len());
        for target in &targets {
            let relative = PathBuf::from(target);
            let loaded = self.load_managed_document(target)?;
            let proposed = loaded.document.replace_value(key, new_value)?;
            changes.push(PlannedFileChange {
                relative_path: relative,
                expected_revision: loaded.revision,
                proposed_bytes: proposed,
            });
        }
        TransactionPlan::new(changes).commit(&self.root)?;
        Ok(MutationSummary {
            affected_files: targets,
            keys: vec![key.to_owned()],
        })
    }

    pub fn redacted_occurrences(&self, key: &str) -> EnvResult<Vec<RedactedOccurrenceReference>> {
        validate_key(key)?;
        let manifest = ManifestStore::for_root(&self.root).load()?;
        let mut occurrences = Vec::new();
        for relative in self.discover(&manifest)? {
            let loaded = self.load_document(&relative)?;
            let matching = loaded
                .document
                .assignments()
                .into_iter()
                .filter(|assignment| assignment.key == key)
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                continue;
            }
            let value = Zeroizing::new(loaded.document.decoded_value(key)?);
            occurrences.push(RedactedOccurrenceReference {
                file: to_manifest_path(&relative),
                value_state: if value.is_empty() {
                    RedactedValueState::Empty
                } else {
                    RedactedValueState::Present
                },
            });
        }
        Ok(occurrences)
    }

    pub fn opaque_copy_impact(&self, file: &str, key: &str) -> EnvResult<Vec<String>> {
        validate_key(key)?;
        let targets = self.value_write_targets(file, key)?;
        for target in &targets {
            let loaded = self.load_managed_document(target)?;
            loaded.document.assignment(key)?;
        }
        Ok(targets)
    }

    pub fn copy_value_from(
        &self,
        source: &ProjectService,
        request: OpaqueValueCopyRequest,
    ) -> EnvResult<MutationSummary> {
        validate_key(&request.key)?;
        if self.project_id == source.project_id {
            return Err(EnvError::invalid(
                "프로젝트 간 복사는 서로 다른 두 등록 프로젝트가 필요합니다.",
            ));
        }

        let source_document = source.load_managed_document(&request.source_file)?;
        let source_value = Zeroizing::new(source_document.document.decoded_value(&request.key)?);
        if source_value.is_empty() {
            return Err(EnvError::invalid(format!(
                "{} 원본 값이 비어 있어 복사할 수 없습니다.",
                request.key
            )));
        }

        let targets = self.value_write_targets(&request.target_file, &request.key)?;
        let mut changes = Vec::with_capacity(targets.len());
        for target in &targets {
            let relative = PathBuf::from(target);
            let loaded = self.load_managed_document(target)?;
            let proposed = loaded
                .document
                .replace_value(&request.key, source_value.as_str())?;
            changes.push(PlannedFileChange {
                relative_path: relative,
                expected_revision: loaded.revision,
                proposed_bytes: proposed,
            });
        }
        TransactionPlan::new(changes).commit(&self.root)?;
        Ok(MutationSummary {
            affected_files: targets,
            keys: vec![request.key],
        })
    }

    pub fn save_description(&self, request: SaveDescriptionRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let assignment = loaded.document.assignment(&request.key)?;
        let assignment_index = assignment.node_index;
        let insertion_start = attached_comment_start(&loaded.document, assignment_index)
            .unwrap_or(assignment.span.start);
        let newline = newline(&loaded.document);
        let replacement = request
            .lines
            .iter()
            .map(|line| format!("# {}{newline}", sanitize_comment(line)))
            .collect::<String>();
        let proposed = loaded.document.replace_span(
            crate::Span::new(insertion_start, assignment.span.start),
            replacement.as_bytes(),
        );
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

    pub(super) fn value_write_targets(&self, file: &str, key: &str) -> EnvResult<Vec<String>> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        let targets = manifest
            .links
            .iter()
            .find(|link| link.key == key && link.members.iter().any(|member| member.file == file))
            .map_or_else(
                || vec![file.to_owned()],
                |link| {
                    link.members
                        .iter()
                        .map(|member| member.file.clone())
                        .collect()
                },
            );
        Ok(targets)
    }

    pub(super) fn checked_value_write_targets(
        &self,
        file: &str,
        key: &str,
    ) -> EnvResult<Vec<String>> {
        let targets = self.value_write_targets(file, key)?;
        for target in &targets {
            let loaded = self.load_managed_document(target)?;
            loaded.document.assignment(key)?;
        }
        Ok(targets)
    }

    pub(super) fn opaque_file_state(&self, relative: &str) -> EnvResult<OpaqueFileState> {
        let relative_path = Path::new(relative);
        let target = safe_existing_target(&self.root, relative_path)?;
        let metadata = fs::metadata(&target).map_err(|error| EnvError::io(relative_path, error))?;
        let modified = metadata
            .modified()
            .map_err(|error| EnvError::io(relative_path, error))?;
        let (modified_before_epoch, modified_seconds, modified_nanoseconds) =
            match modified.duration_since(UNIX_EPOCH) {
                Ok(duration) => (false, duration.as_secs(), duration.subsec_nanos()),
                Err(error) => {
                    let duration = error.duration();
                    (true, duration.as_secs(), duration.subsec_nanos())
                }
            };
        Ok(OpaqueFileState {
            relative_path: relative.to_owned(),
            byte_len: metadata.len(),
            modified_before_epoch,
            modified_seconds,
            modified_nanoseconds,
        })
    }
}

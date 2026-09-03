use super::*;

pub(super) fn build_changes(
    root: &Path,
    entries: &[TeamImportEntry],
    accepted: &BTreeSet<String>,
) -> EnvResult<BTreeMap<String, Zeroizing<Vec<u8>>>> {
    let mut changes = BTreeMap::new();
    for entry in entries {
        let path = to_manifest_path(&entry.target_path);
        let proposed = match &entry.expected {
            ExpectedTarget::Missing => Zeroizing::new(entry.package_bytes.to_vec()),
            ExpectedTarget::Existing(_) => {
                let target = root.join(&entry.target_path);
                let mut proposed = Zeroizing::new(
                    fs::read(&target).map_err(|error| EnvError::io(&entry.target_path, error))?,
                );
                for occurrence in &entry.occurrences {
                    match occurrence.state {
                        TeamImportOccurrenceState::New => {
                            append_assignment(
                                &mut proposed,
                                &occurrence.key,
                                &occurrence.raw_value,
                            );
                        }
                        TeamImportOccurrenceState::Conflict
                            if accepted.contains(&occurrence.id) =>
                        {
                            let document = Document::parse(proposed.to_vec(), &entry.target_path)?;
                            *proposed = document
                                .replace_value(&occurrence.key, &occurrence.decoded_value)?;
                        }
                        TeamImportOccurrenceState::Conflict
                        | TeamImportOccurrenceState::Unchanged => {}
                    }
                }
                proposed
            }
        };
        Document::parse(proposed.to_vec(), &entry.target_path)?;
        changes.insert(path, proposed);
    }
    Ok(changes)
}

pub(super) fn append_assignment(source: &mut Vec<u8>, key: &str, raw_value: &[u8]) {
    if !source.is_empty() && !source.ends_with(b"\n") {
        source.push(b'\n');
    }
    source.extend_from_slice(key.as_bytes());
    source.push(b'=');
    source.extend_from_slice(raw_value);
    source.push(b'\n');
}

pub(super) fn validate_link_invariants(
    root: &Path,
    manifest: &Manifest,
    changes: &BTreeMap<String, Zeroizing<Vec<u8>>>,
) -> EnvResult<()> {
    for link in &manifest.links {
        let mut expected_value: Option<Zeroizing<String>> = None;
        for member in &link.members {
            let relative = PathBuf::from(&member.file);
            let source = if let Some(proposed) = changes.get(&member.file) {
                Zeroizing::new(proposed.to_vec())
            } else {
                let target = root.join(&relative);
                let metadata = fs::symlink_metadata(&target)
                    .map_err(|_| EnvError::link_member_missing(&link.key, &relative))?;
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > MAX_ENTRY_BYTES
                {
                    return Err(EnvError::link_member_missing(&link.key, &relative));
                }
                let canonical = target
                    .canonicalize()
                    .map_err(|_| EnvError::link_member_missing(&link.key, &relative))?;
                if !canonical.starts_with(root) {
                    return Err(EnvError::link_member_missing(&link.key, &relative));
                }
                Zeroizing::new(
                    fs::read(&target)
                        .map_err(|_| EnvError::link_member_missing(&link.key, &relative))?,
                )
            };
            let document = Document::parse(source.to_vec(), &relative)
                .map_err(|_| EnvError::link_member_missing(&link.key, &relative))?;
            let value = Zeroizing::new(
                document
                    .decoded_value(&link.key)
                    .map_err(|_| EnvError::link_member_missing(&link.key, &relative))?,
            );
            if let Some(expected) = &expected_value {
                if expected.as_str() != value.as_str() {
                    return Err(EnvError::link_conflict(&link.key));
                }
            } else {
                expected_value = Some(value);
            }
        }
    }
    Ok(())
}

pub(super) fn apply_entries(
    root: &Path,
    entries: Vec<TeamImportEntry>,
    mut changes: BTreeMap<String, Zeroizing<Vec<u8>>>,
    preview: &TeamImportPreview,
    accepted: &BTreeSet<String>,
) -> EnvResult<TeamImportSummary> {
    for entry in &entries {
        verify_expected(root, entry)?;
    }
    let mut prepared = Vec::new();
    let mut affected_files = Vec::new();
    for entry in entries {
        let path = to_manifest_path(&entry.target_path);
        let changed = matches!(entry.expected, ExpectedTarget::Missing)
            || entry.occurrences.iter().any(|occurrence| {
                occurrence.state == TeamImportOccurrenceState::New
                    || (occurrence.state == TeamImportOccurrenceState::Conflict
                        && accepted.contains(&occurrence.id))
            });
        if !changed {
            continue;
        }
        let proposed = changes.remove(&path).ok_or_else(package_invalid)?;
        let target = root.join(&entry.target_path);
        let parent = target.parent().ok_or_else(package_invalid)?;
        let mut staged = NamedTempFile::new_in(parent)
            .map_err(|error| EnvError::io(&entry.target_path, error))?;
        staged
            .write_all(&proposed)
            .map_err(|error| EnvError::io(&entry.target_path, error))?;
        let permissions = if target.exists() {
            let permissions = fs::metadata(&target)
                .map_err(|error| EnvError::io(&entry.target_path, error))?
                .permissions();
            staged
                .as_file_mut()
                .set_permissions(permissions.clone())
                .map_err(|error| EnvError::io(&entry.target_path, error))?;
            Some(permissions)
        } else {
            None
        };
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|error| EnvError::io(&entry.target_path, error))?;
        let original = match entry.expected {
            ExpectedTarget::Missing => None,
            ExpectedTarget::Existing(_) => Some(Zeroizing::new(
                fs::read(&target).map_err(|error| EnvError::io(&entry.target_path, error))?,
            )),
        };
        affected_files.push(path);
        prepared.push(PreparedImport {
            target,
            relative_path: entry.target_path,
            original,
            permissions,
            staged,
        });
    }
    commit_prepared(prepared)?;
    Ok(TeamImportSummary {
        added_count: preview.new_count,
        updated_count: accepted.len(),
        unchanged_count: preview.unchanged_count,
        kept_local_count: preview.conflict_count.saturating_sub(accepted.len()),
        affected_files,
    })
}

pub(super) fn verify_expected(root: &Path, entry: &TeamImportEntry) -> EnvResult<()> {
    let target = root.join(&entry.target_path);
    match &entry.expected {
        ExpectedTarget::Missing => match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(EnvError::changed_externally(&entry.target_path)),
        },
        ExpectedTarget::Existing(expected) => {
            let metadata = fs::symlink_metadata(&target)
                .map_err(|_| EnvError::changed_externally(&entry.target_path))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(EnvError::changed_externally(&entry.target_path));
            }
            let current =
                fs::read(&target).map_err(|_| EnvError::changed_externally(&entry.target_path))?;
            if &FileRevision::from_bytes(&current) != expected {
                return Err(EnvError::changed_externally(&entry.target_path));
            }
            Ok(())
        }
    }
}

pub(super) struct PreparedImport {
    target: PathBuf,
    relative_path: PathBuf,
    original: Option<Zeroizing<Vec<u8>>>,
    permissions: Option<fs::Permissions>,
    staged: NamedTempFile,
}

pub(super) struct CommittedImport {
    target: PathBuf,
    relative_path: PathBuf,
    original: Option<Zeroizing<Vec<u8>>>,
    permissions: Option<fs::Permissions>,
}

pub(super) fn commit_prepared(prepared: Vec<PreparedImport>) -> EnvResult<()> {
    let mut committed = Vec::new();
    for item in prepared {
        let result = if item.original.is_some() {
            item.staged.persist(&item.target).map(|_| ())
        } else {
            item.staged.persist_noclobber(&item.target).map(|_| ())
        };
        match result {
            Ok(()) => committed.push(CommittedImport {
                target: item.target,
                relative_path: item.relative_path,
                original: item.original,
                permissions: item.permissions,
            }),
            Err(error) => {
                let commit_error = EnvError::io(&item.relative_path, error.error);
                return match rollback(committed) {
                    Ok(()) => Err(commit_error),
                    Err(_) => Err(EnvError::transaction(
                        "공유 패키지 적용과 복구가 완전하지 않을 수 있습니다.",
                    )),
                };
            }
        }
    }
    Ok(())
}

pub(super) fn rollback(committed: Vec<CommittedImport>) -> EnvResult<()> {
    for item in committed.into_iter().rev() {
        if let Some(original) = item.original {
            let parent = item.target.parent().ok_or_else(package_invalid)?;
            let mut staged = NamedTempFile::new_in(parent)
                .map_err(|error| EnvError::io(&item.relative_path, error))?;
            staged
                .write_all(&original)
                .map_err(|error| EnvError::io(&item.relative_path, error))?;
            if let Some(permissions) = item.permissions {
                staged
                    .as_file_mut()
                    .set_permissions(permissions)
                    .map_err(|error| EnvError::io(&item.relative_path, error))?;
            }
            staged
                .as_file_mut()
                .sync_all()
                .map_err(|error| EnvError::io(&item.relative_path, error))?;
            staged
                .persist(&item.target)
                .map_err(|error| EnvError::io(&item.relative_path, error.error))?;
        } else {
            fs::remove_file(&item.target)
                .map_err(|error| EnvError::io(&item.relative_path, error))?;
        }
    }
    Ok(())
}

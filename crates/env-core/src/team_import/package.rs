use super::*;

pub fn plan_encrypted_team_import(
    root: &Path,
    manifest: &Manifest,
    package: &Path,
    passphrase: SecretString,
) -> EnvResult<TeamImportPlan> {
    let root = root
        .canonicalize()
        .map_err(|error| EnvError::io(root, error))?;
    let metadata = fs::symlink_metadata(package).map_err(|error| EnvError::io(package, error))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ENCRYPTED_BYTES
    {
        return Err(package_invalid());
    }
    let encrypted =
        Zeroizing::new(fs::read(package).map_err(|error| EnvError::io(package, error))?);
    let decryptor =
        age::Decryptor::new_buffered(encrypted.as_slice()).map_err(|_| decrypt_failed())?;
    let identity = age::scrypt::Identity::new(passphrase);
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| decrypt_failed())?;
    let mut zip_bytes = Zeroizing::new(Vec::new());
    reader
        .take(MAX_DECRYPTED_ZIP_BYTES + 1)
        .read_to_end(&mut zip_bytes)
        .map_err(|_| decrypt_failed())?;
    if zip_bytes.len() as u64 > MAX_DECRYPTED_ZIP_BYTES {
        return Err(package_invalid());
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(zip_bytes.as_slice())).map_err(|_| package_invalid())?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(package_invalid());
    }

    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(archive.len());
    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|_| package_invalid())?;
        if file.is_dir() || file.is_symlink() || file.size() > MAX_ENTRY_BYTES {
            return Err(package_invalid());
        }
        total_bytes = total_bytes
            .checked_add(file.size())
            .ok_or_else(package_invalid)?;
        if total_bytes > MAX_TOTAL_ENTRY_BYTES {
            return Err(package_invalid());
        }
        let relative = validate_package_path(file.name())?;
        let normalized = to_manifest_path(&relative);
        if !is_managed_import_path(&relative, &normalized, manifest) || !seen.insert(normalized) {
            return Err(package_invalid());
        }
        let mut package_bytes = Zeroizing::new(Vec::with_capacity(file.size() as usize));
        file.by_ref()
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut package_bytes)
            .map_err(|_| package_invalid())?;
        if package_bytes.len() as u64 > MAX_ENTRY_BYTES {
            return Err(package_invalid());
        }
        entries.push(build_entry(
            &root,
            manifest,
            relative.clone(),
            relative,
            package_bytes,
        )?);
    }
    entries.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    validate_package_links(manifest, &entries)?;
    let preview = project_preview(&entries);
    Ok(TeamImportPlan {
        root,
        manifest: manifest.clone(),
        entries,
        preview,
    })
}

pub(super) fn build_entry(
    root: &Path,
    manifest: &Manifest,
    source_path: PathBuf,
    target_path: PathBuf,
    package_bytes: Zeroizing<Vec<u8>>,
) -> EnvResult<TeamImportEntry> {
    let package_document = Document::parse(package_bytes.to_vec(), &source_path)?;
    if !package_document.duplicate_keys().is_empty() || package_document.assignments().is_empty() {
        return Err(package_invalid());
    }
    let (expected, current_document) = inspect_target(root, &target_path)?;
    let mut occurrences = Vec::new();
    for assignment in package_document.assignments() {
        let key = assignment.key.to_owned();
        let decoded_value = Zeroizing::new(package_document.decoded_value(&key)?);
        let state = current_document
            .as_ref()
            .map_or(TeamImportOccurrenceState::New, |current| {
                match current.decoded_value(&key) {
                    Ok(value) if value == *decoded_value => TeamImportOccurrenceState::Unchanged,
                    Ok(_) => TeamImportOccurrenceState::Conflict,
                    Err(_) => TeamImportOccurrenceState::New,
                }
            });
        let link_id = manifest
            .link_for(&to_manifest_path(&target_path), &key)
            .map(|link| link.id.clone());
        occurrences.push(IncomingOccurrence {
            id: occurrence_id(&source_path, &key),
            key,
            decoded_value,
            raw_value: Zeroizing::new(assignment.value_bytes().to_vec()),
            state,
            link_id,
        });
    }
    Ok(TeamImportEntry {
        source_path,
        target_path,
        package_bytes,
        occurrences,
        expected,
    })
}

pub(super) fn validate_package_path(name: &str) -> EnvResult<PathBuf> {
    if name.is_empty() || name.contains('\\') || name.contains('\0') {
        return Err(package_invalid());
    }
    let path = PathBuf::from(name);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(package_invalid());
    }
    let basename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(package_invalid)?;
    if !is_env_candidate(basename) {
        return Err(package_invalid());
    }
    Ok(path)
}

pub(super) fn inspect_target(
    root: &Path,
    relative: &Path,
) -> EnvResult<(ExpectedTarget, Option<Document>)> {
    let target = root.join(relative);
    let parent = target.parent().ok_or_else(package_invalid)?;
    let canonical_parent = parent.canonicalize().map_err(|_| package_invalid())?;
    if !canonical_parent.starts_with(root) {
        return Err(package_invalid());
    }
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > MAX_ENTRY_BYTES
            {
                return Err(package_invalid());
            }
            let canonical = target.canonicalize().map_err(|_| package_invalid())?;
            if !canonical.starts_with(root) {
                return Err(package_invalid());
            }
            let current = fs::read(&target).map_err(|error| EnvError::io(relative, error))?;
            let revision = FileRevision::from_bytes(&current);
            let document = Document::parse(current, relative)?;
            if !document.duplicate_keys().is_empty() {
                return Err(package_invalid());
            }
            Ok((ExpectedTarget::Existing(revision), Some(document)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((ExpectedTarget::Missing, None))
        }
        Err(error) => Err(EnvError::io(relative, error)),
    }
}

pub(super) fn decrypt_failed() -> EnvError {
    EnvError::new(
        EnvErrorCode::PackageDecryptFailed,
        "공유 파일을 열지 못했습니다. 암호와 파일을 확인해주세요.",
    )
}

pub(super) fn package_invalid() -> EnvError {
    EnvError::new(
        EnvErrorCode::PackageInvalid,
        "지원하지 않거나 안전하게 적용할 수 없는 공유 파일입니다.",
    )
}

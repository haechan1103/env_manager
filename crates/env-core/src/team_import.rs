use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

use crate::discovery::{is_env_candidate, to_manifest_path};
use crate::{
    DiscoveryOptions, Document, EnvError, EnvErrorCode, EnvResult, FileRevision, Manifest,
};

const MAX_ENCRYPTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECRYPTED_ZIP_BYTES: u64 = 40 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TeamImportOccurrenceState {
    New,
    Unchanged,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportOccurrenceProjection {
    pub id: String,
    pub key: String,
    pub state: TeamImportOccurrenceState,
    pub link_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportFileProjection {
    pub path: String,
    pub target_path: String,
    pub occurrences: Vec<TeamImportOccurrenceProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportPreview {
    pub files: Vec<TeamImportFileProjection>,
    pub new_count: usize,
    pub unchanged_count: usize,
    pub conflict_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportSummary {
    pub added_count: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub kept_local_count: usize,
    pub affected_files: Vec<String>,
}

pub struct TeamImportPlan {
    root: PathBuf,
    manifest: Manifest,
    entries: Vec<TeamImportEntry>,
    preview: TeamImportPreview,
}

struct TeamImportEntry {
    source_path: PathBuf,
    target_path: PathBuf,
    package_bytes: Zeroizing<Vec<u8>>,
    occurrences: Vec<IncomingOccurrence>,
    expected: ExpectedTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TeamImportValueSide {
    Local,
    Shared,
}

struct IncomingOccurrence {
    id: String,
    key: String,
    decoded_value: Zeroizing<String>,
    raw_value: Zeroizing<Vec<u8>>,
    state: TeamImportOccurrenceState,
    link_id: Option<String>,
}

enum ExpectedTarget {
    Missing,
    Existing(FileRevision),
}

impl TeamImportPlan {
    pub fn preview(&self) -> &TeamImportPreview {
        &self.preview
    }

    pub fn apply(self, shared_conflicts: &[String]) -> EnvResult<TeamImportSummary> {
        let known_conflicts = self
            .entries
            .iter()
            .flat_map(|entry| &entry.occurrences)
            .filter(|occurrence| occurrence.state == TeamImportOccurrenceState::Conflict)
            .map(|occurrence| occurrence.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut accepted = BTreeSet::new();
        for id in shared_conflicts {
            if !known_conflicts.contains(id.as_str()) {
                return Err(package_invalid());
            }
            accepted.insert(id.clone());
        }
        expand_linked_conflicts(&self.entries, &mut accepted);
        let changes = build_changes(&self.root, &self.entries, &accepted)?;
        validate_link_invariants(&self.root, &self.manifest, &changes)?;
        apply_entries(&self.root, self.entries, changes, &self.preview, &accepted)
    }

    pub fn remap_file(
        &mut self,
        source_path: &str,
        target_path: &str,
    ) -> EnvResult<TeamImportPreview> {
        let source = validate_package_path(source_path)?;
        let target = validate_package_path(target_path)?;
        let target_normalized = to_manifest_path(&target);
        if !is_managed_import_path(&target, &target_normalized, &self.manifest) {
            return Err(package_invalid());
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.source_path != source && entry.target_path == target)
        {
            return Err(package_invalid());
        }
        let index = self
            .entries
            .iter()
            .position(|entry| entry.source_path == source)
            .ok_or_else(package_invalid)?;
        let package_bytes = Zeroizing::new(self.entries[index].package_bytes.to_vec());
        let replacement = build_entry(&self.root, &self.manifest, source, target, package_bytes)?;
        let previous = std::mem::replace(&mut self.entries[index], replacement);
        if let Err(error) = validate_package_links(&self.manifest, &self.entries) {
            self.entries[index] = previous;
            return Err(error);
        }
        self.preview = project_preview(&self.entries);
        Ok(self.preview.clone())
    }

    pub fn reveal_conflict(
        &self,
        occurrence_id: &str,
        side: TeamImportValueSide,
    ) -> EnvResult<Zeroizing<String>> {
        let (entry, occurrence) = self
            .entries
            .iter()
            .find_map(|entry| {
                entry
                    .occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == occurrence_id)
                    .map(|occurrence| (entry, occurrence))
            })
            .filter(|(_, occurrence)| occurrence.state == TeamImportOccurrenceState::Conflict)
            .ok_or_else(package_invalid)?;
        match side {
            TeamImportValueSide::Shared => Ok(Zeroizing::new(occurrence.decoded_value.to_string())),
            TeamImportValueSide::Local => {
                verify_expected(&self.root, entry)?;
                let current = Zeroizing::new(
                    fs::read(self.root.join(&entry.target_path))
                        .map_err(|error| EnvError::io(&entry.target_path, error))?,
                );
                let document = Document::parse(current.to_vec(), &entry.target_path)?;
                Ok(Zeroizing::new(document.decoded_value(&occurrence.key)?))
            }
        }
    }
}

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

fn build_entry(
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

fn project_preview(entries: &[TeamImportEntry]) -> TeamImportPreview {
    let files = entries
        .iter()
        .map(|entry| TeamImportFileProjection {
            path: to_manifest_path(&entry.source_path),
            target_path: to_manifest_path(&entry.target_path),
            occurrences: entry
                .occurrences
                .iter()
                .map(|occurrence| TeamImportOccurrenceProjection {
                    id: occurrence.id.clone(),
                    key: occurrence.key.clone(),
                    state: occurrence.state,
                    link_id: occurrence.link_id.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let states = files.iter().flat_map(|file| &file.occurrences);
    TeamImportPreview {
        new_count: states
            .clone()
            .filter(|item| item.state == TeamImportOccurrenceState::New)
            .count(),
        unchanged_count: states
            .clone()
            .filter(|item| item.state == TeamImportOccurrenceState::Unchanged)
            .count(),
        conflict_count: states
            .filter(|item| item.state == TeamImportOccurrenceState::Conflict)
            .count(),
        files,
    }
}

fn validate_package_links(manifest: &Manifest, entries: &[TeamImportEntry]) -> EnvResult<()> {
    for link in &manifest.links {
        let mut values = Vec::new();
        for member in &link.members {
            let occurrence = entries
                .iter()
                .find(|entry| to_manifest_path(&entry.target_path) == member.file)
                .and_then(|entry| {
                    entry
                        .occurrences
                        .iter()
                        .find(|occurrence| occurrence.key == link.key)
                });
            if let Some(occurrence) = occurrence {
                values.push(occurrence.decoded_value.as_str());
            }
        }
        if !values.is_empty() && values.len() != link.members.len() {
            return Err(EnvError::link_member_missing(
                &link.key,
                Path::new("encrypted-share"),
            ));
        }
        if values.windows(2).any(|pair| pair[0] != pair[1]) {
            return Err(EnvError::link_conflict(&link.key));
        }
    }
    Ok(())
}

fn build_changes(
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

fn append_assignment(source: &mut Vec<u8>, key: &str, raw_value: &[u8]) {
    if !source.is_empty() && !source.ends_with(b"\n") {
        source.push(b'\n');
    }
    source.extend_from_slice(key.as_bytes());
    source.push(b'=');
    source.extend_from_slice(raw_value);
    source.push(b'\n');
}

fn expand_linked_conflicts(entries: &[TeamImportEntry], accepted: &mut BTreeSet<String>) {
    let selected_links = entries
        .iter()
        .flat_map(|entry| &entry.occurrences)
        .filter(|occurrence| accepted.contains(&occurrence.id))
        .filter_map(|occurrence| occurrence.link_id.clone())
        .collect::<BTreeSet<_>>();
    for occurrence in entries.iter().flat_map(|entry| &entry.occurrences) {
        if occurrence
            .link_id
            .as_ref()
            .is_some_and(|id| selected_links.contains(id))
            && occurrence.state == TeamImportOccurrenceState::Conflict
        {
            accepted.insert(occurrence.id.clone());
        }
    }
}

fn is_managed_import_path(relative: &Path, normalized: &str, manifest: &Manifest) -> bool {
    let mut options = DiscoveryOptions::default();
    options
        .ignored_files
        .extend(manifest.scan.ignored_files.iter().cloned());
    options
        .ignored_directories
        .extend(manifest.scan.ignored_directories.iter().cloned());
    !options.ignored_files.contains(normalized)
        && !relative.components().any(|component| {
            options
                .ignored_directories
                .contains(&component.as_os_str().to_string_lossy().into_owned())
        })
}

fn validate_link_invariants(
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

fn occurrence_id(relative: &Path, key: &str) -> String {
    let seed = format!("{}\0{key}", to_manifest_path(relative));
    format!("occ-{}", &blake3::hash(seed.as_bytes()).to_hex()[..16])
}

fn validate_package_path(name: &str) -> EnvResult<PathBuf> {
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

fn inspect_target(root: &Path, relative: &Path) -> EnvResult<(ExpectedTarget, Option<Document>)> {
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

fn apply_entries(
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

fn verify_expected(root: &Path, entry: &TeamImportEntry) -> EnvResult<()> {
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

struct PreparedImport {
    target: PathBuf,
    relative_path: PathBuf,
    original: Option<Zeroizing<Vec<u8>>>,
    permissions: Option<fs::Permissions>,
    staged: NamedTempFile,
}

struct CommittedImport {
    target: PathBuf,
    relative_path: PathBuf,
    original: Option<Zeroizing<Vec<u8>>>,
    permissions: Option<fs::Permissions>,
}

fn commit_prepared(prepared: Vec<PreparedImport>) -> EnvResult<()> {
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

fn rollback(committed: Vec<CommittedImport>) -> EnvResult<()> {
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

fn decrypt_failed() -> EnvError {
    EnvError::new(
        EnvErrorCode::PackageDecryptFailed,
        "공유 파일을 열지 못했습니다. 암호와 파일을 확인해주세요.",
    )
}

fn package_invalid() -> EnvError {
    EnvError::new(
        EnvErrorCode::PackageInvalid,
        "지원하지 않거나 안전하게 적용할 수 없는 공유 파일입니다.",
    )
}

#[cfg(test)]
mod tests {
    use age::secrecy::SecretString;
    use env_test_support::SyntheticProject;

    use super::*;
    use crate::{ExportOccurrence, LinkGroup, LinkMember, export_project_env};

    fn encrypted_fixture(
        selection: Option<&[ExportOccurrence]>,
    ) -> (SyntheticProject, PathBuf, SecretString) {
        let source = SyntheticProject::new();
        source.write(
            ".env.local",
            "# sender comment\nTOKEN=fake_team_import_canary\nPORT=fake_3000\n",
        );
        source.write("apps/web/.env.dev", "PORT=fake_4000\n");
        let package = source.root().join("team-env.zip.age");
        let passphrase = SecretString::from("fake-team-passphrase-2026".to_owned());
        export_project_env(
            source.root(),
            &Manifest::default(),
            &package,
            Some(passphrase.clone()),
            selection,
        )
        .expect("package");
        (source, package, passphrase)
    }

    #[test]
    fn applies_new_files_without_exposing_values() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("import plan");
        let preview_json = serde_json::to_string(plan.preview()).expect("preview json");

        assert_eq!(plan.preview().new_count, 3);
        assert!(!preview_json.contains("fake_team_import_canary"));
        let summary = plan.apply(&[]).expect("apply");
        assert_eq!(summary.added_count, 3);
        assert!(
            destination
                .read(".env.local")
                .starts_with(b"# sender comment")
        );
    }

    #[test]
    fn partial_share_merges_only_selected_values_and_preserves_local_content() {
        let selection = vec![ExportOccurrence {
            file: ".env.local".to_owned(),
            key: "TOKEN".to_owned(),
        }];
        let (_source, package, passphrase) = encrypted_fixture(Some(&selection));
        let destination = SyntheticProject::new();
        destination.write(
            ".env.local",
            "# local comment\nTOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\n",
        );

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let conflict_id = plan.preview().files[0].occurrences[0].id.clone();
        let summary = plan.apply(&[conflict_id]).expect("apply shared conflict");

        assert_eq!(summary.updated_count, 1);
        assert_eq!(
            destination.read(".env.local"),
            b"# local comment\nTOKEN=fake_team_import_canary\nLOCAL_ONLY=fake_keep_me\n"
        );
        assert!(!destination.root().join("apps/web/.env.dev").exists());
    }

    #[test]
    fn remaps_one_incoming_file_and_reveals_only_an_explicit_conflict_side() {
        let selection = vec![ExportOccurrence {
            file: ".env.local".to_owned(),
            key: "TOKEN".to_owned(),
        }];
        let (_source, package, passphrase) = encrypted_fixture(Some(&selection));
        let destination = SyntheticProject::new();
        destination.write(
            ".env.staging",
            "TOKEN=fake_staging_local\nLOCAL_ONLY=fake_keep_me\n",
        );

        let mut plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let preview = plan
            .remap_file(".env.local", ".env.staging")
            .expect("remap");
        assert_eq!(preview.files[0].path, ".env.local");
        assert_eq!(preview.files[0].target_path, ".env.staging");
        assert_eq!(preview.conflict_count, 1);

        let conflict_id = preview.files[0].occurrences[0].id.clone();
        let local = plan
            .reveal_conflict(&conflict_id, TeamImportValueSide::Local)
            .expect("local reveal");
        let shared = plan
            .reveal_conflict(&conflict_id, TeamImportValueSide::Shared)
            .expect("shared reveal");
        assert_eq!(local.as_str(), "fake_staging_local");
        assert_eq!(shared.as_str(), "fake_team_import_canary");

        plan.apply(&[conflict_id]).expect("apply remapped value");
        assert_eq!(
            destination.read(".env.staging"),
            b"TOKEN=fake_team_import_canary\nLOCAL_ONLY=fake_keep_me\n"
        );
        assert!(!destination.root().join(".env.local").exists());
    }

    #[test]
    fn rejects_mapping_two_package_files_to_one_target() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");
        let mut plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");

        let result = plan.remap_file(".env.local", "apps/web/.env.dev");
        assert!(matches!(result, Err(error) if error.code() == EnvErrorCode::PackageInvalid));
    }

    #[test]
    fn keeping_a_conflict_preserves_the_local_value_while_adding_new_variables() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        destination.write(
            ".env.local",
            "TOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\n",
        );
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let summary = plan.apply(&[]).expect("keep local conflict");

        assert_eq!(summary.kept_local_count, 1);
        assert_eq!(
            destination.read(".env.local"),
            b"TOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\nPORT=fake_3000\n"
        );
    }

    #[test]
    fn wrong_passphrase_returns_a_value_free_error() {
        let (_source, package, _passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        let result = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            SecretString::from("fake-wrong-passphrase-2026".to_owned()),
        );
        let error = match result {
            Ok(_) => panic!("wrong passphrase must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code(), EnvErrorCode::PackageDecryptFailed);
        assert!(!error.to_string().contains("fake_team_import_canary"));
    }

    #[test]
    fn rejects_files_excluded_by_the_shared_manifest() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        let mut manifest = Manifest::default();
        manifest.scan.ignored_files.push(".env.local".to_owned());
        let result =
            plan_encrypted_team_import(destination.root(), &manifest, &package, passphrase);
        assert!(matches!(result, Err(error) if error.code() == EnvErrorCode::PackageInvalid));
    }

    #[test]
    fn linked_conflicts_are_applied_as_one_group() {
        let source = SyntheticProject::new();
        source.write(".env.local", "TOKEN=fake_shared\n");
        source.write(".env.staging", "TOKEN=fake_shared\n");
        let package = source.root().join("linked.zip.age");
        let passphrase = SecretString::from("fake-linked-passphrase-2026".to_owned());
        export_project_env(
            source.root(),
            &Manifest::default(),
            &package,
            Some(passphrase.clone()),
            None,
        )
        .expect("package");
        let destination = SyntheticProject::new();
        destination.write(".env.local", "TOKEN=fake_old\n");
        destination.write(".env.staging", "TOKEN=fake_old\n");
        let mut manifest = Manifest::default();
        manifest.links.push(LinkGroup {
            id: "token-link".to_owned(),
            key: "TOKEN".to_owned(),
            members: vec![
                LinkMember {
                    file: ".env.local".to_owned(),
                },
                LinkMember {
                    file: ".env.staging".to_owned(),
                },
            ],
        });

        let plan = plan_encrypted_team_import(destination.root(), &manifest, &package, passphrase)
            .expect("plan");
        let selected = plan.preview().files[0].occurrences[0].id.clone();
        let summary = plan.apply(&[selected]).expect("linked apply");
        assert_eq!(summary.updated_count, 2);
        assert_eq!(destination.read(".env.local"), b"TOKEN=fake_shared\n");
        assert_eq!(destination.read(".env.staging"), b"TOKEN=fake_shared\n");
    }

    #[test]
    fn rejects_traversal_and_non_env_entries() {
        for name in [
            "../.env",
            "/.env",
            ".env.example",
            "notes.txt",
            "apps\\web\\.env",
        ] {
            assert!(validate_package_path(name).is_err(), "must reject {name}");
        }
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::discovery::is_env_candidate;
use crate::discovery::to_manifest_path;
use crate::manifest::MANIFEST_FILE_NAME;
use crate::{
    ClassificationReviewProjection, ClassificationReviewReason, ClassificationSource, CodexAccess,
    DiscoveryOptions, Document, EnvError, EnvResult, FileProjection, FileRevision, GroupProjection,
    LinkGroup, LinkMember, Manifest, ManifestStore, Node, OccurrenceProjection, PlannedFileChange,
    ProjectProjection, ProviderValue, RedactedValueState, TransactionPlan, VariablePolicy,
    default_access, detect_client_exposure, discover_env_files, suggest_access,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveValueRequest {
    pub file: String,
    pub key: String,
    pub new_value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDescriptionRequest {
    pub file: String,
    pub key: String,
    pub lines: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvFileRequest {
    pub file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupRequest {
    pub file: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameGroupRequest {
    pub file: String,
    pub current_name: String,
    pub new_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddVariableRequest {
    pub file: String,
    pub key: String,
    pub group: String,
    pub description: Vec<String>,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteVariableRequest {
    pub file: String,
    pub key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveVariableRequest {
    pub file: String,
    pub key: String,
    pub target_group: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRequest {
    pub key: String,
    pub files: Vec<String>,
    pub source_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueValueCopyRequest {
    pub source_file: String,
    pub target_file: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedOccurrenceReference {
    pub file: String,
    pub value_state: RedactedValueState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationSummary {
    pub affected_files: Vec<String>,
    pub keys: Vec<String>,
}

/// Value-free state captured before an opaque stdin value write.
///
/// The guard deliberately stores filesystem metadata rather than a digest of env
/// bytes so the short-lived cross-process plan cannot become a durable value
/// fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedOpaqueValueWrite {
    pub project_id: String,
    pub file: String,
    pub key: String,
    pub affected_files: Vec<String>,
    file_states: Vec<OpaqueFileState>,
    manifest_state: OpaqueFileState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OpaqueFileState {
    relative_path: String,
    byte_len: u64,
    modified_before_epoch: bool,
    modified_seconds: u64,
    modified_nanoseconds: u32,
}

pub struct ProjectService {
    root: PathBuf,
    project_id: String,
}

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

    fn save_value_inner(
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

    pub fn create_group(&self, request: CreateGroupRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let name = sanitize_group(&request.name)?;
        if find_unique_group_index(&loaded.document, &name)?.is_some() {
            return Err(EnvError::invalid("같은 이름의 그룹이 이미 있습니다."));
        }

        let newline = newline(&loaded.document);
        let insert_at = loaded.document.source().len();
        let mut block = String::new();
        if insert_at > content_start(&loaded.document) {
            if loaded.document.source()[insert_at - 1] != b'\n' {
                block.push_str(newline);
            }
            if !loaded.document.source()[..insert_at]
                .ends_with(format!("{newline}{newline}").as_bytes())
            {
                block.push_str(newline);
            }
        }
        block.push_str("# @group ");
        block.push_str(&name);
        block.push_str(newline);

        let proposed = loaded
            .document
            .replace_span(crate::Span::new(insert_at, insert_at), block.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: Vec::new(),
        })
    }

    pub fn rename_group(&self, request: RenameGroupRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let current_index = find_unique_group_index(&loaded.document, &request.current_name)?
            .ok_or_else(|| EnvError::invalid("변경할 그룹을 찾지 못했습니다."))?;
        let new_name = sanitize_group(&request.new_name)?;
        if new_name == request.current_name {
            return Err(EnvError::invalid("현재 그룹 이름과 같습니다."));
        }
        if find_unique_group_index(&loaded.document, &new_name)?.is_some() {
            return Err(EnvError::invalid("같은 이름의 그룹이 이미 있습니다."));
        }
        let Node::GroupDirective {
            name: name_span, ..
        } = loaded.document.nodes()[current_index]
        else {
            unreachable!("group lookup only returns group directives")
        };
        let proposed = loaded.document.replace_span(name_span, new_name.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: Vec::new(),
        })
    }

    pub fn add_variable(&self, request: AddVariableRequest) -> EnvResult<MutationSummary> {
        validate_key(&request.key)?;
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        if loaded
            .document
            .assignments()
            .iter()
            .any(|assignment| assignment.key == request.key)
        {
            return Err(EnvError::duplicate_key(&request.key, &relative));
        }

        let newline = newline(&loaded.document);
        let requested_group = request.group.trim();
        let is_ungrouped = requested_group.is_empty() || requested_group == "기타";
        let normalized_group = if is_ungrouped {
            None
        } else {
            Some(sanitize_group(requested_group)?)
        };
        let existing_group = normalized_group
            .as_deref()
            .map(|name| find_unique_group_index(&loaded.document, name))
            .transpose()?
            .flatten();
        let insert_at = if is_ungrouped {
            first_group_start(&loaded.document).unwrap_or(loaded.document.source().len())
        } else {
            existing_group.map_or(loaded.document.source().len(), |group_index| {
                next_group_start(&loaded.document, group_index)
                    .unwrap_or(loaded.document.source().len())
            })
        };

        let mut block = String::new();
        let has_content_before = insert_at > content_start(&loaded.document);
        if has_content_before && loaded.document.source()[insert_at - 1] != b'\n' {
            block.push_str(newline);
        }
        let previous_has_blank = loaded.document.source()[..insert_at]
            .ends_with(format!("{newline}{newline}").as_bytes());
        if has_content_before && !previous_has_blank {
            block.push_str(newline);
        }
        if existing_group.is_none()
            && let Some(group) = normalized_group
        {
            block.push_str("# @group ");
            block.push_str(&group);
            block.push_str(newline);
            block.push_str(newline);
        }
        for line in &request.description {
            block.push_str("# ");
            block.push_str(&sanitize_comment(line));
            block.push_str(newline);
        }
        block.push_str(&request.key);
        block.push('=');
        block.push_str(&encode_new_value(&request.value));
        block.push_str(newline);

        let proposed = loaded
            .document
            .replace_span(crate::Span::new(insert_at, insert_at), block.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        self.ensure_policy(&request.key)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

    pub fn delete_variable(&self, request: DeleteVariableRequest) -> EnvResult<MutationSummary> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        if manifest.link_for(&request.file, &request.key).is_some() {
            return Err(EnvError::invalid(
                "연결된 변수는 먼저 현재 occurrence를 연결에서 분리해야 삭제할 수 있습니다.",
            ));
        }
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let assignment = loaded.document.assignment(&request.key)?;
        let start = attached_comment_start(&loaded.document, assignment.node_index)
            .unwrap_or(assignment.span.start);
        let proposed = loaded
            .document
            .replace_span(crate::Span::new(start, assignment.span.end), b"");
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

    pub fn move_variable(&self, request: MoveVariableRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let assignment = loaded.document.assignment(&request.key)?;
        let block_start = attached_comment_start(&loaded.document, assignment.node_index)
            .unwrap_or(assignment.span.start);
        let block_end = assignment.span.end;
        let current_group = group_at(&loaded.document, assignment.node_index);
        let target_group = request.target_group.trim();
        if current_group == target_group {
            return Err(EnvError::invalid("이미 선택한 그룹에 있습니다."));
        }

        let target_original = if target_group.is_empty() || target_group == "기타" {
            loaded
                .document
                .nodes()
                .iter()
                .find_map(|node| match node {
                    Node::GroupDirective { span, .. } => Some(span.start),
                    _ => None,
                })
                .unwrap_or(loaded.document.source().len())
        } else {
            let group_index = find_unique_group_index(&loaded.document, target_group)?
                .ok_or_else(|| EnvError::invalid("이동할 그룹을 찾지 못했습니다."))?;
            next_group_start(&loaded.document, group_index)
                .unwrap_or(loaded.document.source().len())
        };

        let block = loaded.document.source()[block_start..block_end].to_vec();
        let mut proposed = loaded.document.source().to_vec();
        proposed.drain(block_start..block_end);
        let removed_len = block_end - block_start;
        let target = if target_original > block_end {
            target_original - removed_len
        } else {
            target_original
        };
        proposed.splice(target..target, block);
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

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

    fn scan_with_manifest(
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

    fn discover(&self, manifest: &Manifest) -> EnvResult<Vec<PathBuf>> {
        let mut options = DiscoveryOptions::default();
        options
            .ignored_files
            .extend(manifest.scan.ignored_files.iter().cloned());
        options
            .ignored_directories
            .extend(manifest.scan.ignored_directories.iter().cloned());
        discover_env_files(&self.root, &options)
    }

    fn load_document(&self, relative: &Path) -> EnvResult<LoadedDocument> {
        let target = safe_existing_target(&self.root, relative)?;
        let bytes = fs::read(&target).map_err(|error| EnvError::io(relative, error))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(EnvError::file_too_large(relative));
        }
        let revision = FileRevision::from_bytes(&bytes);
        let document = Document::parse(bytes, relative)?;
        Ok(LoadedDocument { document, revision })
    }

    fn load_managed_document(&self, file: &str) -> EnvResult<LoadedDocument> {
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

    fn value_write_targets(&self, file: &str, key: &str) -> EnvResult<Vec<String>> {
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

    fn checked_value_write_targets(&self, file: &str, key: &str) -> EnvResult<Vec<String>> {
        let targets = self.value_write_targets(file, key)?;
        for target in &targets {
            let loaded = self.load_managed_document(target)?;
            loaded.document.assignment(key)?;
        }
        Ok(targets)
    }

    fn opaque_file_state(&self, relative: &str) -> EnvResult<OpaqueFileState> {
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

    fn commit_one(
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

    fn ensure_policy(&self, key: &str) -> EnvResult<()> {
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

    fn commit_files_then_manifest(
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

fn files_for_git_safety(projections: &[FileProjection]) -> Vec<PathBuf> {
    projections
        .iter()
        .map(|projection| PathBuf::from(&projection.path))
        .collect()
}

struct LoadedDocument {
    document: Document,
    revision: FileRevision,
}

fn project_file(
    path: &str,
    document: &Document,
    manifest: &Manifest,
    file_labels: &BTreeMap<String, String>,
    unclassified: &mut BTreeSet<String>,
) -> FileProjection {
    let duplicates = document.duplicate_keys();
    let mut groups = vec![GroupProjection {
        name: "기타".to_owned(),
        variables: Vec::new(),
    }];
    let mut current_group = 0;
    let mut pending_comments = Vec::<String>::new();
    let mut warnings = Vec::new();

    for node in document.nodes() {
        match node {
            Node::GroupDirective { name, .. } => {
                groups.push(GroupProjection {
                    name: document.text(*name).to_owned(),
                    variables: Vec::new(),
                });
                current_group = groups.len() - 1;
                pending_comments.clear();
            }
            Node::Comment { content, .. } => {
                let comment = document.text(*content).trim_start().to_owned();
                if comment.starts_with('@') {
                    warnings.push("알 수 없는 Kavranta 지시문이 있습니다.".to_owned());
                    pending_comments.clear();
                } else {
                    pending_comments.push(comment);
                }
            }
            Node::Blank { .. } => pending_comments.clear(),
            Node::Assignment { key, value, .. } => {
                let key = document.text(*key).to_owned();
                let access = manifest.access_for(&key);
                if access == CodexAccess::Unclassified {
                    unclassified.insert(key.clone());
                }
                groups[current_group].variables.push(OccurrenceProjection {
                    value_state: if value.start == value.end {
                        RedactedValueState::Empty
                    } else {
                        RedactedValueState::Present
                    },
                    description: std::mem::take(&mut pending_comments),
                    display_value: None,
                    codex_access: access,
                    linked_count: manifest.linked_count(path, &key),
                    link_id: manifest.link_for(path, &key).map(|link| link.id.clone()),
                    linked_files: manifest.link_for(path, &key).map_or_else(Vec::new, |link| {
                        link.members
                            .iter()
                            .map(|member| member.file.clone())
                            .collect()
                    }),
                    duplicate: duplicates.contains_key(&key),
                    client_exposure: detect_client_exposure(&key),
                    key,
                });
            }
            Node::Opaque { .. } => {
                warnings.push("보존되었지만 해석하지 못한 줄이 있습니다.".to_owned());
                pending_comments.clear();
            }
        }
    }
    if groups
        .first()
        .is_some_and(|group| group.variables.is_empty())
        && groups.len() > 1
    {
        groups.remove(0);
    }
    FileProjection {
        path: path.to_owned(),
        display_name: file_labels
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_owned()),
        groups,
        warnings,
    }
}

fn attached_comment_start(document: &Document, assignment_index: usize) -> Option<usize> {
    let mut cursor = assignment_index;
    let mut start = None;
    while cursor > 0 {
        match &document.nodes()[cursor - 1] {
            Node::Comment { span, content } if !document.text(*content).trim().starts_with('@') => {
                start = Some(span.start);
                cursor -= 1;
            }
            _ => break,
        }
    }
    start
}

fn group_at(document: &Document, node_index: usize) -> &str {
    document.nodes()[..node_index]
        .iter()
        .rev()
        .find_map(|node| match node {
            Node::GroupDirective { name, .. } => Some(document.text(*name)),
            _ => None,
        })
        .unwrap_or("기타")
}

fn find_unique_group_index(document: &Document, group: &str) -> EnvResult<Option<usize>> {
    let mut matches = document
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node {
            Node::GroupDirective { name, .. } if document.text(*name) == group => Some(index),
            _ => None,
        });
    let found = matches.next();
    if matches.next().is_some() {
        return Err(EnvError::invalid(format!(
            "{group} 그룹이 파일에 여러 번 있어 대상을 정할 수 없습니다."
        )));
    }
    Ok(found)
}

fn first_group_start(document: &Document) -> Option<usize> {
    document.nodes().iter().find_map(|node| match node {
        Node::GroupDirective { span, .. } => Some(span.start),
        _ => None,
    })
}

fn next_group_start(document: &Document, group_index: usize) -> Option<usize> {
    document.nodes()[group_index + 1..]
        .iter()
        .find_map(|node| match node {
            Node::GroupDirective { span, .. } => Some(span.start),
            _ => None,
        })
}

fn newline(document: &Document) -> &'static str {
    match document.newline_style() {
        crate::NewlineStyle::Lf => "\n",
        crate::NewlineStyle::CrLf => "\r\n",
    }
}

fn content_start(document: &Document) -> usize {
    usize::from(document.has_bom()) * 3
}

fn sanitize_comment(line: &str) -> String {
    line.replace(['\r', '\n'], " ").trim().to_owned()
}

fn sanitize_group(group: &str) -> EnvResult<String> {
    let sanitized = group.replace(['\r', '\n'], " ").trim().to_owned();
    if sanitized.is_empty() || sanitized.starts_with('@') || sanitized == "기타" {
        return Err(EnvError::invalid("그룹 이름이 올바르지 않습니다."));
    }
    Ok(sanitized)
}

fn validate_key(key: &str) -> EnvResult<()> {
    let mut bytes = key.bytes();
    let Some(first) = bytes.next() else {
        return Err(EnvError::invalid("변수 이름이 비어 있습니다."));
    };
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(EnvError::invalid("변수 이름 형식이 올바르지 않습니다."));
    }
    Ok(())
}

fn encode_new_value(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value
        .bytes()
        .all(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'#' | b'\'' | b'"'))
    {
        return value.to_owned();
    }
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

fn safe_existing_target(root: &Path, relative: &Path) -> EnvResult<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(EnvError::path_outside(relative));
    }
    let joined = root.join(relative);
    let target = joined
        .canonicalize()
        .map_err(|error| EnvError::io(relative, error))?;
    if !target.starts_with(root) {
        return Err(EnvError::path_outside(relative));
    }
    Ok(target)
}

fn safe_new_env_target(
    root: &Path,
    relative: &Path,
    options: &DiscoveryOptions,
) -> EnvResult<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(EnvError::path_outside(relative));
    }
    let name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| EnvError::invalid("생성할 env 파일 이름이 올바르지 않습니다."))?;
    if !is_env_candidate(name) {
        return Err(EnvError::invalid(
            "새 파일은 지원되는 env 형식(.env, .env.*, *.env*, .dev.vars, .dev.vars.*)이어야 하며 example 파일은 만들 수 없습니다.",
        ));
    }
    let manifest_path = to_manifest_path(relative);
    if options.ignored_files.contains(&manifest_path) {
        return Err(EnvError::invalid("manifest에서 제외한 env 파일입니다."));
    }
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    if parent.components().any(|component| {
        options
            .ignored_directories
            .contains(&component.as_os_str().to_string_lossy().into_owned())
    }) {
        return Err(EnvError::invalid(
            "관리 제외 디렉터리에는 env 파일을 만들 수 없습니다.",
        ));
    }
    let parent_target = root
        .join(parent)
        .canonicalize()
        .map_err(|error| EnvError::io(parent, error))?;
    if !parent_target.starts_with(root) || !parent_target.is_dir() {
        return Err(EnvError::path_outside(relative));
    }
    let target = parent_target.join(name);
    match fs::symlink_metadata(&target) {
        Ok(_) => Err(EnvError::invalid("같은 경로의 파일이 이미 있습니다.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(EnvError::io(relative, error)),
    }
}

#[cfg(test)]
mod tests {
    use env_test_support::SyntheticProject;

    use super::*;

    #[test]
    fn initialization_classifies_without_exposing_values() {
        let project = SyntheticProject::new();
        project.write(
            ".env.local",
            "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\nPORT=fake_3000\nCUSTOM=fake_value\n",
        );
        let service = ProjectService::open(project.root()).expect("service");
        let projection = service.initialize().expect("initialize");

        assert_eq!(projection.files.len(), 1);
        let variables = &projection.files[0].groups[0].variables;
        assert_eq!(variables[0].codex_access, CodexAccess::Protected);
        assert_eq!(variables[1].codex_access, CodexAccess::Unclassified);
        assert_eq!(variables[2].codex_access, CodexAccess::Unclassified);
        assert_eq!(projection.unclassified_count, 2);
        assert_eq!(projection.access_review_count, 0);
        assert!(variables.iter().all(|item| item.display_value.is_none()));
    }

    #[test]
    fn initialization_revokes_only_legacy_heuristic_allows() {
        let project = SyntheticProject::new();
        project.write(".env.local", "PORT=fake_3000\nHOST=fake_localhost\n");
        let service = ProjectService::open(project.root()).expect("service");
        let store = ManifestStore::for_root(project.root());
        let mut manifest = store.load().expect("manifest");
        manifest.variables.insert(
            "PORT".to_owned(),
            VariablePolicy {
                codex_access: CodexAccess::ReadWrite,
                classified_by: ClassificationSource::Heuristic,
            },
        );
        manifest.variables.insert(
            "HOST".to_owned(),
            VariablePolicy {
                codex_access: CodexAccess::ReadWrite,
                classified_by: ClassificationSource::User,
            },
        );
        store.save(&manifest).expect("seed manifest");

        let projection = service.initialize().expect("initialize");
        let policies = ManifestStore::for_root(project.root())
            .load()
            .expect("migrated manifest");

        assert_eq!(policies.access_for("PORT"), CodexAccess::Unclassified);
        assert_eq!(policies.access_for("HOST"), CodexAccess::ReadWrite);
        assert_eq!(projection.unclassified_count, 1);
    }

    #[test]
    fn public_secret_name_is_the_only_name_based_access_review_exception() {
        let project = SyntheticProject::new();
        project.write(
            ".env.local",
            "NEXT_PUBLIC_API_KEY=fake_secret\nAPI_KEY=fake_secret\nCUSTOM_MODE=fake_value\n",
        );
        let service = ProjectService::open(project.root()).expect("service");

        let projection = service.initialize().expect("initialize");
        let requiring_review = projection
            .classification_review
            .iter()
            .filter(|item| !item.review_reasons.is_empty())
            .collect::<Vec<_>>();

        assert_eq!(projection.access_review_count, 1);
        assert_eq!(requiring_review[0].key, "NEXT_PUBLIC_API_KEY");
        assert_eq!(
            requiring_review[0].review_reasons,
            vec![ClassificationReviewReason::ClientExposureConflict]
        );
    }

    #[test]
    fn local_file_display_name_changes_projection_without_renaming_the_file() {
        let project = SyntheticProject::new();
        project.write("apps/web/.env.local", "PORT=fake_3000\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        let path = service
            .validate_file_for_display_name("apps/web/.env.local")
            .expect("valid display target");
        let projection = service
            .initialize_with_file_labels(&BTreeMap::from([(path, "Web local".to_owned())]))
            .expect("scan");
        assert_eq!(projection.files[0].display_name, "Web local");
        assert_eq!(projection.files[0].path, "apps/web/.env.local");
        assert!(project.root().join("apps/web/.env.local").is_file());
        assert!(!project.root().join("apps/web/Web local").exists());
    }

    #[test]
    fn creates_empty_env_file_only_inside_an_existing_project_directory() {
        let project = SyntheticProject::new();
        fs::create_dir_all(project.root().join("apps/mobile")).expect("fixture directory");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        service
            .create_env_file(CreateEnvFileRequest {
                file: "apps/mobile/.env".to_owned(),
            })
            .expect("create env file");
        service
            .create_env_file(CreateEnvFileRequest {
                file: "apps/mobile/.dev.vars.staging".to_owned(),
            })
            .expect("create Wrangler env file");

        assert_eq!(project.read("apps/mobile/.env"), b"");
        assert_eq!(project.read("apps/mobile/.dev.vars.staging"), b"");
        let projection = service.scan().expect("scan");
        assert_eq!(
            projection
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["apps/mobile/.dev.vars.staging", "apps/mobile/.env"]
        );
    }

    #[test]
    fn refuses_example_overwrite_and_path_escape_for_new_env_files() {
        let project = SyntheticProject::new();
        project.write(".env", "PORT=fake_3000\n");
        fs::create_dir_all(project.root().join("node_modules/pkg"))
            .expect("excluded fixture directory");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        for file in [".env.example", ".env", "../.env", "node_modules/pkg/.env"] {
            assert!(
                service
                    .create_env_file(CreateEnvFileRequest {
                        file: file.to_owned(),
                    })
                    .is_err(),
                "must reject {file}"
            );
        }
        assert_eq!(project.read(".env"), b"PORT=fake_3000\n");
    }

    #[test]
    fn linked_save_updates_all_members() {
        let project = SyntheticProject::new();
        project.write(".env.local", "PORT=fake_3000\n");
        project.write(".env.development", "PORT=\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .create_link(LinkRequest {
                key: "PORT".to_owned(),
                files: vec![".env.local".to_owned(), ".env.development".to_owned()],
                source_file: None,
            })
            .expect("link");
        service
            .save_value(SaveValueRequest {
                file: ".env.local".to_owned(),
                key: "PORT".to_owned(),
                new_value: "fake_4000".to_owned(),
            })
            .expect("save");

        assert_eq!(project.read(".env.local"), b"PORT=fake_4000\n");
        assert_eq!(project.read(".env.development"), b"PORT=fake_4000\n");
    }

    #[test]
    fn copies_a_protected_value_between_projects_without_returning_it() {
        let source = SyntheticProject::new();
        let target = SyntheticProject::new();
        let canary = "fake_CROSS_PROJECT_CANARY_41";
        source.write(".env.local", &format!("GEMINI_API_KEY={canary}\n"));
        target.write(".env.local", "GEMINI_API_KEY=\n");
        target.write(".env.development", "GEMINI_API_KEY=\n");
        let source_service = ProjectService::open(source.root()).expect("source service");
        let target_service = ProjectService::open(target.root()).expect("target service");
        source_service.initialize().expect("source initialize");
        target_service.initialize().expect("target initialize");
        target_service
            .create_link(LinkRequest {
                key: "GEMINI_API_KEY".to_owned(),
                files: vec![".env.local".to_owned(), ".env.development".to_owned()],
                source_file: None,
            })
            .expect("target link");

        let candidates = source_service
            .redacted_occurrences("GEMINI_API_KEY")
            .expect("redacted candidates");
        let serialized = serde_json::to_string(&candidates).expect("serialize candidates");
        assert!(!serialized.contains(canary));
        assert_eq!(candidates[0].value_state, RedactedValueState::Present);

        let summary = target_service
            .copy_value_from(
                &source_service,
                OpaqueValueCopyRequest {
                    source_file: ".env.local".to_owned(),
                    target_file: ".env.local".to_owned(),
                    key: "GEMINI_API_KEY".to_owned(),
                },
            )
            .expect("opaque copy");

        assert_eq!(
            summary.affected_files,
            vec![".env.development".to_owned(), ".env.local".to_owned()]
        );
        assert_eq!(
            target.read(".env.local"),
            format!("GEMINI_API_KEY={canary}\n").as_bytes()
        );
        assert_eq!(
            target.read(".env.development"),
            format!("GEMINI_API_KEY={canary}\n").as_bytes()
        );
        assert!(
            !serde_json::to_string(&summary)
                .expect("serialize summary")
                .contains(canary)
        );
    }

    #[test]
    fn provider_values_require_managed_unique_non_empty_names() {
        let project = SyntheticProject::new();
        project.write(
            ".env.production",
            "API_KEY=fake_provider_secret\nAPI_HOST=fake_api_host\nEMPTY=\n",
        );
        project.write("notes.env", "OTHER=fake_other\n");
        project.write("notes.txt", "UNMANAGED=fake_unmanaged\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        let selected = service
            .provider_values(
                ".env.production",
                &["API_KEY".to_owned(), "API_HOST".to_owned()],
            )
            .expect("provider values");
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].key(), "API_KEY");
        assert_eq!(selected[0].value(), "fake_provider_secret");

        assert!(
            service
                .provider_values(".env.production", &["EMPTY".to_owned()])
                .is_err()
        );
        assert!(
            service
                .provider_values(
                    ".env.production",
                    &["API_KEY".to_owned(), "API_KEY".to_owned()],
                )
                .is_err()
        );
        assert_eq!(
            service
                .provider_values("notes.env", &["OTHER".to_owned()])
                .expect("suffix env value")[0]
                .key(),
            "OTHER"
        );
        assert!(
            service
                .provider_values("notes.txt", &["UNMANAGED".to_owned()])
                .is_err()
        );
    }

    #[test]
    fn link_supports_four_peer_occurrences() {
        let project = SyntheticProject::new();
        project.write(".env", "PORT=fake_3000\n");
        project.write(".env.local", "PORT=\n");
        project.write(".env.dev", "PORT=\n");
        project.write("apps/web/.env.local", "PORT=\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        let files = vec![
            ".env".to_owned(),
            ".env.local".to_owned(),
            ".env.dev".to_owned(),
            "apps/web/.env.local".to_owned(),
        ];
        let summary = service
            .create_link(LinkRequest {
                key: "PORT".to_owned(),
                files: files.clone(),
                source_file: Some(".env".to_owned()),
            })
            .expect("link");
        assert_eq!(summary.affected_files.len(), 4);
        service
            .save_value(SaveValueRequest {
                file: ".env.dev".to_owned(),
                key: "PORT".to_owned(),
                new_value: "fake_4100".to_owned(),
            })
            .expect("save");
        for file in files {
            assert_eq!(project.read(&file), b"PORT=fake_4100\n");
        }
    }

    #[test]
    fn adds_variable_inside_existing_group_without_duplicate_marker() {
        let project = SyntheticProject::new();
        project.write(
            ".env.local",
            "# @group GPT\nGPT_MODEL=fake_model\n\n# @group App\nPORT=fake_3000\n",
        );
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .add_variable(AddVariableRequest {
                file: ".env.local".to_owned(),
                key: "GPT_TIMEOUT".to_owned(),
                group: "GPT".to_owned(),
                description: vec!["fake timeout description".to_owned()],
                value: "fake_30".to_owned(),
            })
            .expect("add");

        let output = String::from_utf8(project.read(".env.local")).expect("utf8");
        assert_eq!(output.matches("# @group GPT").count(), 1);
        assert!(
            output.find("GPT_TIMEOUT").expect("new key")
                < output.find("# @group App").expect("next group")
        );
    }

    #[test]
    fn creates_and_renames_an_explicit_empty_group() {
        let project = SyntheticProject::new();
        project.write(".env", "PORT=fake_3000\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        service
            .create_group(CreateGroupRequest {
                file: ".env".to_owned(),
                name: "GPT".to_owned(),
            })
            .expect("create group");
        assert_eq!(project.read(".env"), b"PORT=fake_3000\n\n# @group GPT\n");

        service
            .rename_group(RenameGroupRequest {
                file: ".env".to_owned(),
                current_name: "GPT".to_owned(),
                new_name: "OpenAI".to_owned(),
            })
            .expect("rename group");
        assert_eq!(project.read(".env"), b"PORT=fake_3000\n\n# @group OpenAI\n");
    }

    #[test]
    fn creates_group_in_bom_only_file_without_leading_blank_lines() {
        let project = SyntheticProject::new();
        project.write(".env", "\u{feff}");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .create_group(CreateGroupRequest {
                file: ".env".to_owned(),
                name: "GPT".to_owned(),
            })
            .expect("create group");

        assert_eq!(project.read(".env"), b"\xEF\xBB\xBF# @group GPT\n");
    }

    #[test]
    fn refuses_ambiguous_or_reserved_group_names() {
        let project = SyntheticProject::new();
        project.write(".env", "# @group GPT\nA=fake_a\n# @group GPT\nB=fake_b\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");

        let duplicate = service
            .rename_group(RenameGroupRequest {
                file: ".env".to_owned(),
                current_name: "GPT".to_owned(),
                new_name: "OpenAI".to_owned(),
            })
            .expect_err("duplicate group must be ambiguous");
        assert!(duplicate.to_string().contains("여러 번"));

        let reserved = service
            .create_group(CreateGroupRequest {
                file: ".env".to_owned(),
                name: "기타".to_owned(),
            })
            .expect_err("virtual group name is reserved");
        assert!(reserved.to_string().contains("올바르지"));
    }

    #[test]
    fn adds_ungrouped_variable_before_the_first_group_marker() {
        let project = SyntheticProject::new();
        project.write(".env", "# @group GPT\nGPT_MODEL=fake_model\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .add_variable(AddVariableRequest {
                file: ".env".to_owned(),
                key: "PORT".to_owned(),
                group: "기타".to_owned(),
                description: Vec::new(),
                value: "fake_3000".to_owned(),
            })
            .expect("add ungrouped");

        assert_eq!(
            project.read(".env"),
            b"PORT=fake_3000\n# @group GPT\nGPT_MODEL=fake_model\n"
        );
    }

    #[test]
    fn deletes_assignment_and_attached_description_only() {
        let project = SyntheticProject::new();
        project.write(
            ".env",
            "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\n# keep\nPORT=fake_3000\n",
        );
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .delete_variable(DeleteVariableRequest {
                file: ".env".to_owned(),
                key: "GPT_API_KEY".to_owned(),
            })
            .expect("delete");
        assert_eq!(
            project.read(".env"),
            b"# @group GPT\n# keep\nPORT=fake_3000\n"
        );
    }

    #[test]
    fn moves_assignment_with_description_without_reading_value() {
        let project = SyntheticProject::new();
        project.write(
            ".env",
            "# @group GPT\n# fake description\nGPT_API_KEY=fake_secret\n# @group App\nPORT=fake_3000\n",
        );
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        service
            .move_variable(MoveVariableRequest {
                file: ".env".to_owned(),
                key: "GPT_API_KEY".to_owned(),
                target_group: "App".to_owned(),
            })
            .expect("move");
        assert_eq!(
            project.read(".env"),
            b"# @group GPT\n# @group App\nPORT=fake_3000\n# fake description\nGPT_API_KEY=fake_secret\n"
        );
    }

    #[test]
    fn scan_classifies_newly_added_names() {
        let project = SyntheticProject::new();
        project.write(".env", "PORT=fake_3000\n");
        let service = ProjectService::open(project.root()).expect("service");
        service.initialize().expect("initialize");
        project.write(".env", "PORT=fake_3000\nNEW_CLIENT_SECRET=fake_secret\n");

        let projection = service.scan().expect("scan");
        let variables = &projection.files[0].groups[0].variables;
        assert_eq!(variables[1].codex_access, CodexAccess::Protected);
    }
}

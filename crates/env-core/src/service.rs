use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::to_manifest_path;
use crate::manifest::MANIFEST_FILE_NAME;
use crate::{
    ClassificationSource, CodexAccess, DiscoveryOptions, Document, EnvError, EnvResult,
    FileProjection, FileRevision, GroupProjection, LinkGroup, LinkMember, Manifest, ManifestStore,
    Node, OccurrenceProjection, PlannedFileChange, ProjectProjection, RedactedValueState,
    TransactionPlan, VariablePolicy, discover_env_files, suggest_access,
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
pub struct SaveGroupRequest {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationSummary {
    pub affected_files: Vec<String>,
    pub keys: Vec<String>,
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
        let store = ManifestStore::for_root(&self.root);
        let mut manifest = store.load()?;
        let files = self.discover(&manifest)?;
        let mut changed = false;

        for relative in &files {
            let loaded = self.load_document(relative)?;
            for assignment in loaded.document.assignments() {
                if manifest.variables.contains_key(assignment.key) {
                    continue;
                }
                let suggestion = suggest_access(assignment.key);
                manifest.variables.insert(
                    assignment.key.to_owned(),
                    VariablePolicy {
                        codex_access: suggestion.access,
                        classified_by: ClassificationSource::Heuristic,
                    },
                );
                changed = true;
            }
        }

        if changed || !self.root.join(MANIFEST_FILE_NAME).exists() {
            store.save(&manifest)?;
        }
        self.scan_with_manifest(&manifest)
    }

    pub fn scan(&self) -> EnvResult<ProjectProjection> {
        self.initialize()
    }

    pub fn save_value(&self, request: SaveValueRequest) -> EnvResult<MutationSummary> {
        validate_key(&request.key)?;
        let manifest = ManifestStore::for_root(&self.root).load()?;
        let targets = manifest
            .links
            .iter()
            .find(|link| {
                link.key == request.key
                    && link
                        .members
                        .iter()
                        .any(|member| member.file == request.file)
            })
            .map_or_else(
                || vec![request.file.clone()],
                |link| {
                    link.members
                        .iter()
                        .map(|member| member.file.clone())
                        .collect()
                },
            );
        let mut changes = Vec::with_capacity(targets.len());
        for target in &targets {
            let relative = PathBuf::from(target);
            let loaded = self.load_document(&relative)?;
            let proposed = loaded
                .document
                .replace_value(&request.key, &request.new_value)?;
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

    pub fn save_group(&self, request: SaveGroupRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let name_span = loaded
            .document
            .nodes()
            .iter()
            .find_map(|node| match node {
                Node::GroupDirective { name, .. }
                    if loaded.document.text(*name) == request.current_name =>
                {
                    Some(*name)
                }
                _ => None,
            })
            .ok_or_else(|| EnvError::invalid("변경할 그룹을 찾지 못했습니다."))?;
        let new_name = sanitize_group(&request.new_name)?;
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
        let existing_group =
            loaded
                .document
                .nodes()
                .iter()
                .enumerate()
                .find_map(|(index, node)| match node {
                    Node::GroupDirective { name, .. }
                        if loaded.document.text(*name) == requested_group =>
                    {
                        Some(index)
                    }
                    _ => None,
                });
        let insert_at = existing_group.map_or(loaded.document.source().len(), |group_index| {
            loaded.document.nodes()[group_index + 1..]
                .iter()
                .find_map(|node| match node {
                    Node::GroupDirective { span, .. } => Some(span.start),
                    _ => None,
                })
                .unwrap_or(loaded.document.source().len())
        });

        let mut block = String::new();
        if insert_at > 0 && loaded.document.source()[insert_at - 1] != b'\n' {
            block.push_str(newline);
        }
        let previous_has_blank = loaded.document.source()[..insert_at]
            .ends_with(format!("{newline}{newline}").as_bytes());
        if !previous_has_blank {
            block.push_str(newline);
        }
        if existing_group.is_none() && !requested_group.is_empty() && requested_group != "기타" {
            block.push_str("# @group ");
            block.push_str(&sanitize_group(requested_group)?);
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
            let group_index = loaded
                .document
                .nodes()
                .iter()
                .enumerate()
                .find_map(|(index, node)| match node {
                    Node::GroupDirective { name, .. }
                        if loaded.document.text(*name) == target_group =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
                .ok_or_else(|| EnvError::invalid("이동할 그룹을 찾지 못했습니다."))?;
            loaded.document.nodes()[group_index + 1..]
                .iter()
                .find_map(|node| match node {
                    Node::GroupDirective { span, .. } => Some(span.start),
                    _ => None,
                })
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

    fn scan_with_manifest(&self, manifest: &Manifest) -> EnvResult<ProjectProjection> {
        let files = self.discover(manifest)?;
        let mut projections = Vec::with_capacity(files.len());
        let mut unclassified = BTreeSet::new();
        let mut issue_count = 0;

        for relative in files {
            let loaded = self.load_document(&relative)?;
            let path = to_manifest_path(&relative);
            let projection = project_file(&path, &loaded.document, manifest, &mut unclassified);
            issue_count += projection.warnings.len();
            projections.push(projection);
        }

        Ok(ProjectProjection {
            project_id: self.project_id.clone(),
            name: self.root.file_name().map_or_else(
                || "Project".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            ),
            files: projections,
            unclassified_count: unclassified.len(),
            issue_count,
        })
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
        let suggestion = suggest_access(key);
        manifest.variables.insert(
            key.to_owned(),
            VariablePolicy {
                codex_access: suggestion.access,
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

struct LoadedDocument {
    document: Document,
    revision: FileRevision,
}

fn project_file(
    path: &str,
    document: &Document,
    manifest: &Manifest,
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
                    warnings.push("알 수 없는 Env Manager 지시문이 있습니다.".to_owned());
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

fn newline(document: &Document) -> &'static str {
    match document.newline_style() {
        crate::NewlineStyle::Lf => "\n",
        crate::NewlineStyle::CrLf => "\r\n",
    }
}

fn sanitize_comment(line: &str) -> String {
    line.replace(['\r', '\n'], " ").trim().to_owned()
}

fn sanitize_group(group: &str) -> EnvResult<String> {
    let sanitized = group.replace(['\r', '\n'], " ").trim().to_owned();
    if sanitized.is_empty() || sanitized.starts_with('@') {
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
        assert_eq!(variables[1].codex_access, CodexAccess::ReadWrite);
        assert_eq!(variables[2].codex_access, CodexAccess::Unclassified);
        assert!(variables.iter().all(|item| item.display_value.is_none()));
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

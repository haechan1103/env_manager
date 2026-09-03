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

mod access;
mod links;
mod migration_export;
mod persistence;
mod project;
mod structure;
mod values;

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
#[path = "service/tests.rs"]
mod tests;

use serde::{Deserialize, Serialize};

use crate::{
    ClassificationSource, ClassificationSuggestion, ClientExposureWarning, GitSafetyProjection,
    manifest::CodexAccess,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedactedValueState {
    Empty,
    Present,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OccurrenceProjection {
    pub key: String,
    pub description: Vec<String>,
    pub value_state: RedactedValueState,
    pub display_value: Option<String>,
    pub codex_access: CodexAccess,
    pub linked_count: usize,
    pub link_id: Option<String>,
    pub linked_files: Vec<String>,
    pub duplicate: bool,
    pub client_exposure: Option<ClientExposureWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationReviewProjection {
    pub key: String,
    pub files: Vec<String>,
    pub access: CodexAccess,
    pub classified_by: ClassificationSource,
    pub suggestion: ClassificationSuggestion,
    pub client_exposed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupProjection {
    pub name: String,
    pub variables: Vec<OccurrenceProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProjection {
    pub path: String,
    pub groups: Vec<GroupProjection>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjection {
    pub project_id: String,
    pub name: String,
    pub files: Vec<FileProjection>,
    pub unclassified_count: usize,
    pub issue_count: usize,
    pub git_safety: GitSafetyProjection,
    pub classification_review: Vec<ClassificationReviewProjection>,
    pub client_exposure_count: usize,
}

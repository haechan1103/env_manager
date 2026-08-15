use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider_adapter::AdapterStatus;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentProviderId {
    GithubActions,
    CloudflareWorkers,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GitHubEntryKind {
    Secret,
    Variable,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelection {
    pub key: String,
    pub kind: GitHubEntryKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPushRequest {
    pub provider: DeploymentProviderId,
    pub file: String,
    pub selections: Vec<ProviderSelection>,
    pub repository: Option<String>,
    pub github_environment: Option<String>,
    pub worker: Option<String>,
    pub cloudflare_environment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProviderStatus {
    pub id: DeploymentProviderId,
    pub name: &'static str,
    pub available: bool,
    pub detail: &'static str,
    pub adapter: Option<AdapterStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryOptions {
    pub repositories: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryContext {
    pub repository: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubEnvironmentOptions {
    pub repository: String,
    pub environments: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareTargetContext {
    pub worker: Option<String>,
    pub environments: Vec<String>,
    pub config_path: Option<String>,
    pub account_id: Option<String>,
    pub environment_account_ids: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloudflareAuthState {
    Authenticated,
    NotAuthenticated,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloudflareAccountState {
    Matched,
    Mismatch,
    Ambiguous,
    Unconfigured,
    Unchecked,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloudflareTargetState {
    Accessible,
    Unavailable,
    Unchecked,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareAccessContext {
    pub auth_state: CloudflareAuthState,
    pub auth_type: Option<String>,
    pub account_state: CloudflareAccountState,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub account_count: usize,
    pub target_state: CloudflareTargetState,
    pub adapter: AdapterStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPushResult {
    pub provider: DeploymentProviderId,
    pub pushed_count: usize,
    pub failed_keys: Vec<String>,
}

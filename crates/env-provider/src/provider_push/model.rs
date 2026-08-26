use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider_adapter::AdapterStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialProviderId {
    GithubActions,
    CloudflareWorkers,
    ExpoEas,
    AwsSecretsManager,
    AwsSsmParameterStore,
}

pub const GITHUB_ACTIONS_ID: &str = "github-actions";
pub const CLOUDFLARE_WORKERS_ID: &str = "cloudflare-workers";
pub const EXPO_EAS_ID: &str = "expo-eas";
pub const AWS_SECRETS_MANAGER_ID: &str = "aws-secrets-manager";
pub const AWS_SSM_PARAMETER_STORE_ID: &str = "aws-ssm-parameter-store";

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderEntryKind {
    Secret,
    Variable,
    Plaintext,
    Sensitive,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelection {
    pub key: String,
    pub kind: ProviderEntryKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPushRequest {
    pub provider: String,
    pub file: String,
    pub selections: Vec<ProviderSelection>,
    pub repository: Option<String>,
    pub github_environment: Option<String>,
    pub worker: Option<String>,
    pub cloudflare_environment: Option<String>,
    pub eas_project: Option<String>,
    pub eas_environments: Vec<String>,
    pub personal_target: Option<String>,
    pub aws_profile: Option<String>,
    pub aws_region: Option<String>,
    pub aws_path_prefix: Option<String>,
    pub aws_kms_key_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCompareRequest {
    pub provider: String,
    pub file: String,
    pub keys: Vec<String>,
    pub aws_profile: Option<String>,
    pub aws_region: Option<String>,
    pub aws_path_prefix: Option<String>,
    pub runtime_target_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderComparisonState {
    Same,
    Different,
    Unset,
    Unverifiable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderComparisonItem {
    pub key: String,
    pub remote_name: String,
    pub state: ProviderComparisonState,
    pub result_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCompareResult {
    pub provider: String,
    pub target: String,
    pub items: Vec<ProviderComparisonItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProviderStatus {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub detail: String,
    pub source: DeploymentProviderSource,
    pub version: Option<String>,
    pub target_label: Option<String>,
    pub adapter: Option<AdapterStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentProviderSource {
    Official,
    Personal,
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

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EasTargetContext {
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub environments: Vec<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EasAccessContext {
    pub project: String,
    pub project_id: String,
    pub adapter: AdapterStatus,
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
    pub provider: String,
    pub pushed_count: usize,
    pub failed_keys: Vec<String>,
}

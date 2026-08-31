use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPackManifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub pack_version: String,
    pub action_protocol_version: String,
    #[serde(flatten)]
    pub action: ActionDefinition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ActionDefinition {
    Cli {
        executable_candidates: Vec<String>,
        version_args: Vec<String>,
        profiles: Vec<CliActionProfile>,
        secret_binding: String,
        secret_transport: CliSecretTransport,
        result_policy: CliResultPolicy,
        timeout_seconds: u64,
    },
    Http {
        method: HttpActionMethod,
        url: String,
        secret_bindings: BTreeMap<String, HttpSecretBinding>,
        result_policy: HttpResultPolicy,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliActionProfile {
    pub id: String,
    pub version_requirement: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CliSecretTransport {
    Stdin,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliResultPolicy {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub exit_code: bool,
    #[serde(default = "default_true")]
    pub duration: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpActionMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HttpSecretSource {
    Header,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpSecretBinding {
    pub source: HttpSecretSource,
    #[serde(default)]
    pub name: Option<String>,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpResultPolicy {
    #[serde(default = "default_true")]
    pub status: bool,
    #[serde(default = "default_true")]
    pub duration: bool,
    #[serde(default)]
    pub body: bool,
    #[serde(default)]
    pub success_status_codes: Vec<u16>,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    Cli,
    Http,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionBindingInfo {
    pub id: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPackInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub pack_version: String,
    pub kind: ActionKind,
    pub available: bool,
    pub bindings: Vec<ActionBindingInfo>,
    pub target: String,
    pub cli_version: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionExecutionRequest {
    pub pack_id: String,
    pub file: String,
    pub bindings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionExecutionResult {
    pub pack_id: String,
    pub kind: ActionKind,
    pub succeeded: bool,
    pub status_code: Option<u16>,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub result_code: String,
}

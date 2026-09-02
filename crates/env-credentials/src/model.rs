use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountMetadata {
    pub id: String,
    pub display_name: String,
    pub service: String,
    #[serde(default)]
    pub granted_project_ids: BTreeSet<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountProjection {
    pub id: String,
    pub display_name: String,
    pub service: String,
    pub allowed_for_project: bool,
    pub allowed_project_count: usize,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetadataRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub accounts: Vec<AccountMetadata>,
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self {
            schema_version: 1,
            accounts: Vec::new(),
        }
    }
}

pub struct AccountSecret {
    pub username: Zeroizing<String>,
    pub password: Zeroizing<String>,
}

pub struct CreateAccountInput {
    pub display_name: String,
    pub service: String,
    pub username: Zeroizing<String>,
    pub password: Zeroizing<String>,
    pub grant_project_id: Option<String>,
}

pub struct UpdateAccountInput {
    pub account_id: String,
    pub display_name: String,
    pub service: String,
    pub username: Option<Zeroizing<String>>,
    pub password: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountSecretField {
    Username,
    Password,
}

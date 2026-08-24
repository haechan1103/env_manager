use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::provider_push::cli::{find_cli, provider_command};
use crate::provider_push::{OfficialProviderId, ProviderPushError};

const BUNDLED_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../config/provider-compatibility.json"
));
const MAX_VERSION_OUTPUT: usize = 64 * 1024;
const MAX_LOCAL_REPAIR_SIZE: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterSource {
    Bundled,
    LocalRepair,
    Personal,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterStrategy {
    GhSecretSetV1,
    WranglerSecretBulkV1,
}

#[derive(Debug, Clone)]
pub struct ResolvedAdapter {
    pub executable: PathBuf,
    pub client_version: Version,
    pub profile_id: String,
    pub adapter_version: String,
    pub strategy: AdapterStrategy,
    pub source: AdapterSource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterStatus {
    pub cli_version: String,
    pub profile_id: String,
    pub adapter_version: String,
    pub adapter_source: AdapterSource,
}

impl From<&ResolvedAdapter> for AdapterStatus {
    fn from(value: &ResolvedAdapter) -> Self {
        Self {
            cli_version: value.client_version.to_string(),
            profile_id: value.profile_id.clone(),
            adapter_version: value.adapter_version.clone(),
            adapter_source: value.source,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    catalog_version: String,
    provider_protocol_version: String,
    last_reviewed: String,
    providers: Vec<CatalogProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogProvider {
    id: String,
    display_name: String,
    adapter_version: Option<String>,
    ui_support: String,
    agent_support: String,
    transport: String,
    client: CatalogClient,
    value_transport: String,
    capabilities: Vec<String>,
    official_docs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogClient {
    name: String,
    runtime_probe: String,
    profiles: Vec<CatalogProfile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogProfile {
    id: String,
    strategy: AdapterStrategy,
    version_requirement: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalRepair {
    schema_version: u32,
    provider_id: String,
    base_catalog_version: String,
    base_adapter_version: String,
    profile_id: String,
    client_version_requirement: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ProbeCacheKey {
    executable: PathBuf,
    fingerprint: FileFingerprint,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct FileFingerprint {
    executable_len: u64,
    executable_modified_nanos: u128,
    target_len: u64,
    target_modified_nanos: u128,
    package_len: u64,
    package_modified_nanos: u128,
}

static VERSION_CACHE: OnceLock<Mutex<HashMap<ProbeCacheKey, Version>>> = OnceLock::new();

pub fn resolve(
    provider: OfficialProviderId,
    project_root: &Path,
    app_data: &Path,
) -> Result<ResolvedAdapter, ProviderPushError> {
    let catalog = parse_catalog()?;
    let provider_id = provider_id(provider);
    let definition = catalog
        .providers
        .iter()
        .find(|candidate| candidate.id == provider_id)
        .ok_or_else(catalog_error)?;
    let adapter_version = definition
        .adapter_version
        .as_deref()
        .ok_or_else(catalog_error)?;
    let executable = find_cli(&definition.client.name, project_root).ok_or(ProviderPushError {
        code: "PROVIDER_CLI_NOT_FOUND",
        message: "배포 서비스 CLI를 찾지 못했습니다.",
    })?;
    let client_version = cached_client_version(&executable)?;

    if let Some(profile) = compatible_profile(&definition.client.profiles, &client_version) {
        return Ok(ResolvedAdapter {
            executable,
            client_version,
            profile_id: profile.id.clone(),
            adapter_version: adapter_version.to_owned(),
            strategy: profile.strategy,
            source: AdapterSource::Bundled,
        });
    }

    if let Some(profile) = compatible_local_repair(
        app_data,
        definition,
        &catalog.catalog_version,
        &client_version,
    ) {
        return Ok(ResolvedAdapter {
            executable,
            client_version,
            profile_id: profile.id,
            adapter_version: adapter_version.to_owned(),
            strategy: profile.strategy,
            source: AdapterSource::LocalRepair,
        });
    }

    Err(ProviderPushError {
        code: "PROVIDER_CLI_UNSUPPORTED",
        message: "설치된 CLI 버전과 호환되는 안전한 Adapter를 찾지 못했습니다.",
    })
}

fn parse_catalog() -> Result<Catalog, ProviderPushError> {
    let catalog: Catalog = serde_json::from_str(BUNDLED_CATALOG).map_err(|_| catalog_error())?;
    if catalog.schema_version != 2
        || Version::parse(&catalog.catalog_version).is_err()
        || Version::parse(&catalog.provider_protocol_version).is_err()
    {
        return Err(catalog_error());
    }
    // These fields are part of the closed, validated on-disk contract. Reading them here keeps
    // accidental schema drift visible to the compiler without widening the runtime behavior.
    let _ = &catalog.last_reviewed;
    for provider in &catalog.providers {
        let _ = (
            &provider.display_name,
            &provider.ui_support,
            &provider.agent_support,
            &provider.transport,
            &provider.value_transport,
            &provider.capabilities,
            &provider.official_docs,
            &provider.client.runtime_probe,
        );
    }
    Ok(catalog)
}

fn compatible_profile<'a>(
    profiles: &'a [CatalogProfile],
    client_version: &Version,
) -> Option<&'a CatalogProfile> {
    profiles.iter().find(|profile| {
        VersionReq::parse(&profile.version_requirement)
            .is_ok_and(|requirement| requirement.matches(client_version))
    })
}

fn compatible_local_repair(
    app_data: &Path,
    provider: &CatalogProvider,
    catalog_version: &str,
    client_version: &Version,
) -> Option<CatalogProfile> {
    let adapter_version = provider.adapter_version.as_deref()?;
    let path = app_data
        .join("provider-adapters")
        .join("local")
        .join(format!("{}.json", provider.id));
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_LOCAL_REPAIR_SIZE {
        return None;
    }
    let repair: LocalRepair = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if repair.schema_version != 1
        || repair.provider_id != provider.id
        || repair.base_catalog_version != catalog_version
        || repair.base_adapter_version != adapter_version
    {
        return None;
    }
    let base_profile = provider
        .client
        .profiles
        .iter()
        .find(|profile| profile.id == repair.profile_id)?;
    let requirement = VersionReq::parse(&repair.client_version_requirement).ok()?;
    requirement.matches(client_version).then(|| CatalogProfile {
        id: base_profile.id.clone(),
        strategy: base_profile.strategy,
        version_requirement: repair.client_version_requirement,
    })
}

fn cached_client_version(executable: &Path) -> Result<Version, ProviderPushError> {
    let key = ProbeCacheKey {
        executable: executable.to_path_buf(),
        fingerprint: executable_fingerprint(executable)?,
    };
    let cache = VERSION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(version) = cache.get(&key)
    {
        return Ok(version.clone());
    }

    let args = ["--version".into()];
    let output = provider_command(executable, &args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| cli_unavailable())?;
    if !output.status.success() || output.stdout.len() > MAX_VERSION_OUTPUT {
        return Err(cli_unavailable());
    }
    let version = parse_client_version(&output.stdout).ok_or_else(cli_unavailable)?;
    if let Ok(mut cache) = cache.lock() {
        cache.retain(|candidate, _| candidate.executable != key.executable);
        cache.insert(key, version.clone());
    }
    Ok(version)
}

fn parse_client_version(output: &[u8]) -> Option<Version> {
    let output = std::str::from_utf8(output).ok()?;
    output
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+'))
        })
        .filter(|candidate| {
            candidate
                .chars()
                .next()
                .is_some_and(|item| item.is_ascii_digit())
        })
        .find_map(|candidate| Version::parse(candidate.trim_matches('-')).ok())
}

fn executable_fingerprint(executable: &Path) -> Result<FileFingerprint, ProviderPushError> {
    let executable_metadata = std::fs::metadata(executable).map_err(|_| cli_unavailable())?;
    let target_metadata = std::fs::canonicalize(executable)
        .ok()
        .and_then(|path| std::fs::metadata(path).ok());
    let package_metadata =
        wrangler_package_json(executable).and_then(|path| std::fs::metadata(path).ok());
    Ok(FileFingerprint {
        executable_len: executable_metadata.len(),
        executable_modified_nanos: modified_nanos(&executable_metadata),
        target_len: target_metadata.as_ref().map_or(0, Metadata::len),
        target_modified_nanos: target_metadata.as_ref().map_or(0, modified_nanos),
        package_len: package_metadata.as_ref().map_or(0, Metadata::len),
        package_modified_nanos: package_metadata.as_ref().map_or(0, modified_nanos),
    })
}

fn wrangler_package_json(executable: &Path) -> Option<PathBuf> {
    let bin = executable.parent()?;
    if bin.file_name()?.to_string_lossy() != ".bin" {
        return None;
    }
    Some(bin.parent()?.join("wrangler/package.json"))
}

fn modified_nanos(metadata: &Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn provider_id(provider: OfficialProviderId) -> &'static str {
    match provider {
        OfficialProviderId::GithubActions => "github-actions",
        OfficialProviderId::CloudflareWorkers => "cloudflare-workers",
        OfficialProviderId::AwsSecretsManager => "aws-secrets-manager",
        OfficialProviderId::AwsSsmParameterStore => "aws-ssm-parameter-store",
    }
}

fn cli_unavailable() -> ProviderPushError {
    ProviderPushError {
        code: "PROVIDER_CLI_UNSUPPORTED",
        message: "배포 서비스 CLI 버전을 확인하지 못했습니다.",
    }
}

fn catalog_error() -> ProviderPushError {
    ProviderPushError {
        code: "PROVIDER_ADAPTER_INVALID",
        message: "Provider Adapter 카탈로그가 올바르지 않습니다.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_realistic_cli_versions() {
        assert_eq!(
            parse_client_version(b"gh version 2.78.0 (2026-08-01)"),
            Some(Version::new(2, 78, 0))
        );
        assert_eq!(
            parse_client_version(b"wrangler 4.115.0\n"),
            Some(Version::new(4, 115, 0))
        );
        assert_eq!(parse_client_version(b"not a version"), None);
    }

    #[test]
    fn bundled_catalog_is_closed_and_resolvable() {
        let catalog = parse_catalog().expect("catalog");
        let cloudflare = catalog
            .providers
            .iter()
            .find(|provider| provider.id == "cloudflare-workers")
            .expect("cloudflare");
        let profile = compatible_profile(&cloudflare.client.profiles, &Version::new(4, 115, 0))
            .expect("profile");
        assert_eq!(profile.strategy, AdapterStrategy::WranglerSecretBulkV1);
        assert!(compatible_profile(&cloudflare.client.profiles, &Version::new(5, 0, 0)).is_none());
    }

    #[test]
    fn stale_or_unknown_local_repairs_are_ignored() {
        let directory = tempfile::tempdir().expect("tempdir");
        let local = directory.path().join("provider-adapters/local");
        std::fs::create_dir_all(&local).expect("directory");
        std::fs::write(
            local.join("cloudflare-workers.json"),
            r#"{
              "schemaVersion": 1,
              "providerId": "cloudflare-workers",
              "baseCatalogVersion": "0.0.0",
              "baseAdapterVersion": "1.0.0",
              "profileId": "wrangler-secret-bulk-v1",
              "clientVersionRequirement": ">=5.0.0,<6.0.0"
            }"#,
        )
        .expect("repair");
        let catalog = parse_catalog().expect("catalog");
        let provider = catalog
            .providers
            .iter()
            .find(|provider| provider.id == "cloudflare-workers")
            .expect("provider");
        assert!(
            compatible_local_repair(
                directory.path(),
                provider,
                &catalog.catalog_version,
                &Version::new(5, 0, 0)
            )
            .is_none()
        );
    }

    #[test]
    fn current_local_repair_can_only_reuse_a_bundled_strategy() {
        let directory = tempfile::tempdir().expect("tempdir");
        let local = directory.path().join("provider-adapters/local");
        std::fs::create_dir_all(&local).expect("directory");
        let catalog = parse_catalog().expect("catalog");
        std::fs::write(
            local.join("cloudflare-workers.json"),
            format!(
                r#"{{
              "schemaVersion": 1,
              "providerId": "cloudflare-workers",
              "baseCatalogVersion": "{}",
              "baseAdapterVersion": "1.0.0",
              "profileId": "wrangler-secret-bulk-v1",
              "clientVersionRequirement": ">=5.0.0,<6.0.0"
            }}"#,
                catalog.catalog_version
            ),
        )
        .expect("repair");
        let provider = catalog
            .providers
            .iter()
            .find(|provider| provider.id == "cloudflare-workers")
            .expect("provider");
        let repair = compatible_local_repair(
            directory.path(),
            provider,
            &catalog.catalog_version,
            &Version::new(5, 1, 0),
        )
        .expect("repair");

        assert_eq!(repair.id, "wrangler-secret-bulk-v1");
        assert_eq!(repair.strategy, AdapterStrategy::WranglerSecretBulkV1);
    }

    #[test]
    fn executable_fingerprint_changes_when_the_client_changes() {
        let directory = tempfile::tempdir().expect("tempdir");
        let executable = directory.path().join("provider");
        std::fs::write(&executable, "v1").expect("first executable");
        let before = executable_fingerprint(&executable).expect("before");
        std::fs::write(&executable, "version-two").expect("second executable");
        let after = executable_fingerprint(&executable).expect("after");

        assert_ne!(before, after);
    }
}

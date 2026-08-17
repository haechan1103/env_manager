use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs::{self, Metadata};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::provider_push::ProviderPushError;
use crate::provider_push::cli::{find_cli, provider_command};

const SCHEMA_VERSION: u32 = 1;
const PROVIDER_PROTOCOL_VERSION: &str = "0.2.0";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_VERSION_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_PACKS: usize = 100;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalProviderManifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub provider_protocol_version: String,
    pub value_transport: PersonalValueTransport,
    pub target: Option<PersonalTargetSpec>,
    pub cli: PersonalCliSpec,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PersonalValueTransport {
    Stdin,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalTargetSpec {
    pub label: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalCliSpec {
    pub executable_candidates: Vec<String>,
    pub version_args: Vec<String>,
    pub profiles: Vec<PersonalCliProfile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalCliProfile {
    pub id: String,
    pub version_requirement: String,
    pub push_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalProviderPackInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub target_label: Option<String>,
    pub available: bool,
    pub cli_version: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPersonalProvider {
    pub manifest: PersonalProviderManifest,
    pub executable: PathBuf,
    pub cli_version: Version,
    pub profile: PersonalCliProfile,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct VersionCacheKey {
    executable: PathBuf,
    len: u64,
    modified_nanos: u128,
    version_args: Vec<String>,
}

static VERSION_CACHE: OnceLock<Mutex<HashMap<VersionCacheKey, Version>>> = OnceLock::new();

pub fn install(
    source: &Path,
    app_data: &Path,
    replace: bool,
) -> Result<PersonalProviderPackInfo, ProviderPushError> {
    let source = if source.is_dir() {
        source.join("provider.json")
    } else {
        source.to_path_buf()
    };
    let manifest = read_manifest(&source)?;
    validate_manifest(&manifest)?;
    let directory = packs_directory(app_data);
    fs::create_dir_all(&directory).map_err(|_| storage_error())?;
    let destination = directory.join(format!("{}.json", manifest.id));
    if destination.exists() && !replace {
        return Err(ProviderPushError {
            code: "PERSONAL_PROVIDER_EXISTS",
            message: "같은 ID의 Personal Provider Pack이 이미 설치되어 있습니다.",
        });
    }

    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| invalid_pack())?;
    let mut staging = tempfile::NamedTempFile::new_in(&directory).map_err(|_| storage_error())?;
    staging.write_all(&bytes).map_err(|_| storage_error())?;
    staging.as_file().sync_all().map_err(|_| storage_error())?;
    if replace && destination.exists() {
        fs::remove_file(&destination).map_err(|_| storage_error())?;
    }
    staging.persist(&destination).map_err(|_| storage_error())?;
    Ok(pack_info(&manifest, None))
}

pub fn remove(id: &str, app_data: &Path) -> Result<(), ProviderPushError> {
    validate_id(id)?;
    let path = packs_directory(app_data).join(format!("{id}.json"));
    if !path.exists() {
        return Err(ProviderPushError {
            code: "PERSONAL_PROVIDER_NOT_FOUND",
            message: "설치된 Personal Provider Pack을 찾지 못했습니다.",
        });
    }
    reject_symlink(&path)?;
    fs::remove_file(path).map_err(|_| storage_error())
}

pub fn list(root: &Path, app_data: &Path) -> Vec<PersonalProviderPackInfo> {
    let directory = packs_directory(app_data);
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut manifests = entries
        .flatten()
        .take(MAX_PACKS)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return None;
            }
            let manifest = read_manifest(&path).ok()?;
            validate_manifest(&manifest).ok()?;
            (path.file_stem().and_then(|value| value.to_str()) == Some(&manifest.id))
                .then_some(manifest)
        })
        .collect::<Vec<_>>();
    manifests.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    manifests
        .iter()
        .map(|manifest| {
            let resolved = resolve_manifest(manifest.clone(), root).ok();
            pack_info(manifest, resolved.as_ref())
        })
        .collect()
}

pub fn resolve(
    id: &str,
    root: &Path,
    app_data: &Path,
) -> Result<ResolvedPersonalProvider, ProviderPushError> {
    validate_id(id)?;
    let path = packs_directory(app_data).join(format!("{id}.json"));
    let manifest = read_manifest(&path)?;
    if manifest.id != id {
        return Err(invalid_pack());
    }
    validate_manifest(&manifest)?;
    resolve_manifest(manifest, root)
}

fn resolve_manifest(
    manifest: PersonalProviderManifest,
    root: &Path,
) -> Result<ResolvedPersonalProvider, ProviderPushError> {
    let executable = manifest
        .cli
        .executable_candidates
        .iter()
        .find_map(|candidate| find_executable(candidate, root))
        .ok_or(ProviderPushError {
            code: "PROVIDER_CLI_NOT_FOUND",
            message: "Personal Provider Pack에 필요한 CLI를 찾지 못했습니다.",
        })?;
    let cli_version = probe_version(&executable, &manifest.cli.version_args)?;
    let profile = manifest
        .cli
        .profiles
        .iter()
        .find(|profile| {
            VersionReq::parse(&profile.version_requirement)
                .is_ok_and(|requirement| requirement.matches(&cli_version))
        })
        .cloned()
        .ok_or(ProviderPushError {
            code: "PROVIDER_CLI_UNSUPPORTED",
            message: "설치된 CLI 버전과 호환되는 Personal Provider Profile이 없습니다.",
        })?;
    Ok(ResolvedPersonalProvider {
        manifest,
        executable,
        cli_version,
        profile,
    })
}

fn read_manifest(path: &Path) -> Result<PersonalProviderManifest, ProviderPushError> {
    reject_symlink(path)?;
    let metadata = fs::metadata(path).map_err(|_| invalid_pack())?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(invalid_pack());
    }
    let mut file = fs::File::open(path).map_err(|_| invalid_pack())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_pack())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(invalid_pack());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_pack())
}

fn validate_manifest(manifest: &PersonalProviderManifest) -> Result<(), ProviderPushError> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.provider_protocol_version != PROVIDER_PROTOCOL_VERSION
        || Version::parse(&manifest.version).is_err()
    {
        return Err(invalid_pack());
    }
    validate_id(&manifest.id)?;
    validate_text(&manifest.display_name, 1, 64)?;
    validate_text(&manifest.description, 0, 240)?;

    if manifest.cli.executable_candidates.is_empty()
        || manifest.cli.executable_candidates.len() > 8
        || manifest.cli.version_args.is_empty()
        || manifest.cli.version_args.len() > 8
        || manifest.cli.profiles.is_empty()
        || manifest.cli.profiles.len() > 8
    {
        return Err(invalid_pack());
    }
    for candidate in &manifest.cli.executable_candidates {
        validate_executable_candidate(candidate)?;
    }
    for argument in &manifest.cli.version_args {
        validate_literal_argument(argument, 128)?;
    }

    let target_placeholder = manifest
        .target
        .as_ref()
        .map(|target| {
            validate_text(&target.label, 1, 48)?;
            if !is_identifier(&target.placeholder) || target.placeholder == "key" {
                return Err(invalid_pack());
            }
            Ok(target.placeholder.as_str())
        })
        .transpose()?;

    let mut profile_ids = BTreeSet::new();
    for profile in &manifest.cli.profiles {
        if !is_kebab_identifier(&profile.id)
            || !profile_ids.insert(profile.id.as_str())
            || VersionReq::parse(&profile.version_requirement).is_err()
            || profile.push_args.is_empty()
            || profile.push_args.len() > 32
        {
            return Err(invalid_pack());
        }
        let mut has_key = false;
        let mut has_target = false;
        for argument in &profile.push_args {
            validate_text(argument, 1, 256)?;
            for placeholder in placeholders(argument)? {
                if placeholder == "key" {
                    has_key = true;
                } else if Some(placeholder.as_str()) == target_placeholder {
                    has_target = true;
                } else {
                    return Err(invalid_pack());
                }
            }
        }
        if !has_key || has_target != target_placeholder.is_some() {
            return Err(invalid_pack());
        }
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ProviderPushError> {
    let valid = id.len() <= 96
        && id.starts_with("local.")
        && id.split('.').count() >= 3
        && id.split('.').all(is_kebab_identifier);
    if valid { Ok(()) } else { Err(invalid_pack()) }
}

fn validate_executable_candidate(candidate: &str) -> Result<(), ProviderPushError> {
    validate_text(candidate, 1, 260)?;
    let path = Path::new(candidate);
    if !path.is_absolute() && (path.components().count() != 1 || candidate.contains(['/', '\\'])) {
        return Err(invalid_pack());
    }
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    const BLOCKED: &[&str] = &[
        "sh",
        "bash",
        "zsh",
        "fish",
        "cmd",
        "powershell",
        "pwsh",
        "python",
        "python3",
        "node",
        "bun",
        "deno",
        "ruby",
        "perl",
    ];
    if BLOCKED.contains(&name.as_str()) {
        return Err(ProviderPushError {
            code: "PERSONAL_PROVIDER_UNSAFE_EXECUTABLE",
            message: "셸이나 범용 인터프리터는 Personal Provider CLI로 사용할 수 없습니다.",
        });
    }
    Ok(())
}

fn validate_literal_argument(argument: &str, max: usize) -> Result<(), ProviderPushError> {
    validate_text(argument, 1, max)?;
    if argument.contains('{') || argument.contains('}') {
        return Err(invalid_pack());
    }
    Ok(())
}

fn validate_text(value: &str, min: usize, max: usize) -> Result<(), ProviderPushError> {
    if value.len() < min
        || value.len() > max
        || value.contains('\0')
        || value.contains('\n')
        || value.contains('\r')
    {
        return Err(invalid_pack());
    }
    Ok(())
}

fn placeholders(argument: &str) -> Result<Vec<String>, ProviderPushError> {
    let mut values = Vec::new();
    let mut remaining = argument;
    loop {
        if let Some(close) = remaining.find('}')
            && remaining.find('{').is_none_or(|open| close < open)
        {
            return Err(invalid_pack());
        }
        let Some(open) = remaining.find('{') else {
            break;
        };
        let after = &remaining[open + 1..];
        let close = after.find('}').ok_or_else(invalid_pack)?;
        let name = &after[..close];
        if !is_identifier(name) {
            return Err(invalid_pack());
        }
        values.push(name.to_owned());
        remaining = &after[close + 1..];
    }
    if remaining.contains('}') {
        return Err(invalid_pack());
    }
    Ok(values)
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_kebab_identifier(value: &str) -> bool {
    is_identifier(value)
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn find_executable(candidate: &str, root: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(candidate);
    let resolved = if path.is_absolute() {
        path.is_file().then_some(path)
    } else {
        find_cli(candidate, root)
    }?;
    (!resolved.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
    }))
    .then_some(resolved)
}

fn probe_version(executable: &Path, version_args: &[String]) -> Result<Version, ProviderPushError> {
    let metadata = fs::metadata(executable).map_err(|_| cli_unavailable())?;
    let key = VersionCacheKey {
        executable: executable.to_path_buf(),
        len: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        version_args: version_args.to_vec(),
    };
    let cache = VERSION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(version) = guard.get(&key)
    {
        return Ok(version.clone());
    }
    let args = version_args.iter().map(OsString::from).collect::<Vec<_>>();
    let output = provider_command(executable, &args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| cli_unavailable())?;
    if !output.status.success() || output.stdout.len() > MAX_VERSION_OUTPUT_BYTES {
        return Err(cli_unavailable());
    }
    let version = parse_semantic_version(&output.stdout).ok_or_else(cli_unavailable)?;
    if let Ok(mut guard) = cache.lock() {
        guard.retain(|existing, _| existing.executable != key.executable);
        guard.insert(key, version.clone());
    }
    Ok(version)
}

fn parse_semantic_version(output: &[u8]) -> Option<Version> {
    let output = std::str::from_utf8(output).ok()?;
    output
        .split(|character: char| {
            character.is_whitespace() || matches!(character, ',' | '(' | ')' | ':' | '=')
        })
        .filter_map(|token| Version::parse(token.trim_start_matches(['v', 'V'])).ok())
        .next()
}

fn pack_info(
    manifest: &PersonalProviderManifest,
    resolved: Option<&ResolvedPersonalProvider>,
) -> PersonalProviderPackInfo {
    PersonalProviderPackInfo {
        id: manifest.id.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        target_label: manifest.target.as_ref().map(|target| target.label.clone()),
        available: resolved.is_some(),
        cli_version: resolved.map(|value| value.cli_version.to_string()),
        profile_id: resolved.map(|value| value.profile.id.clone()),
    }
}

fn packs_directory(app_data: &Path) -> PathBuf {
    app_data.join("provider-packs").join("personal")
}

fn reject_symlink(path: &Path) -> Result<(), ProviderPushError> {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(invalid_pack());
    }
    Ok(())
}

fn modified_nanos(metadata: &Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn invalid_pack() -> ProviderPushError {
    ProviderPushError {
        code: "PERSONAL_PROVIDER_INVALID",
        message: "Personal Provider Pack 형식이나 보안 규칙이 올바르지 않습니다.",
    }
}

fn storage_error() -> ProviderPushError {
    ProviderPushError {
        code: "PERSONAL_PROVIDER_STORAGE_FAILED",
        message: "Personal Provider Pack을 로컬 저장소에 반영하지 못했습니다.",
    }
}

fn cli_unavailable() -> ProviderPushError {
    ProviderPushError {
        code: "PROVIDER_CLI_UNSUPPORTED",
        message: "Personal Provider CLI 버전을 안전하게 확인하지 못했습니다.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> PersonalProviderManifest {
        PersonalProviderManifest {
            schema_version: 1,
            id: "local.example.deploy".to_owned(),
            display_name: "Example Deploy".to_owned(),
            description: "Synthetic test provider".to_owned(),
            version: "1.0.0".to_owned(),
            provider_protocol_version: "0.2.0".to_owned(),
            value_transport: PersonalValueTransport::Stdin,
            target: Some(PersonalTargetSpec {
                label: "Application".to_owned(),
                placeholder: "target".to_owned(),
            }),
            cli: PersonalCliSpec {
                executable_candidates: vec!["fake-provider".to_owned()],
                version_args: vec!["--version".to_owned()],
                profiles: vec![PersonalCliProfile {
                    id: "fake-v1".to_owned(),
                    version_requirement: ">=1.0.0,<2.0.0".to_owned(),
                    push_args: vec![
                        "secret".to_owned(),
                        "set".to_owned(),
                        "{key}".to_owned(),
                        "--app={target}".to_owned(),
                    ],
                }],
            },
        }
    }

    #[test]
    fn accepts_closed_stdin_only_manifest() {
        validate_manifest(&valid_manifest()).expect("valid pack");
    }

    #[test]
    fn rejects_value_placeholder_and_shell() {
        let mut value = valid_manifest();
        value.cli.profiles[0].push_args.push("{value}".to_owned());
        assert_eq!(
            validate_manifest(&value)
                .expect_err("value placeholder")
                .code,
            "PERSONAL_PROVIDER_INVALID"
        );

        let mut shell = valid_manifest();
        shell.cli.executable_candidates = vec!["bash".to_owned()];
        assert_eq!(
            validate_manifest(&shell).expect_err("shell").code,
            "PERSONAL_PROVIDER_UNSAFE_EXECUTABLE"
        );
    }

    #[test]
    fn install_is_local_and_requires_explicit_replace() {
        let source = tempfile::tempdir().expect("source");
        let app_data = tempfile::tempdir().expect("app data");
        let manifest = valid_manifest();
        fs::write(
            source.path().join("provider.json"),
            serde_json::to_vec(&manifest).expect("serialize"),
        )
        .expect("write");
        install(source.path(), app_data.path(), false).expect("install");
        assert_eq!(
            install(source.path(), app_data.path(), false)
                .expect_err("duplicate")
                .code,
            "PERSONAL_PROVIDER_EXISTS"
        );
        install(source.path(), app_data.path(), true).expect("replace");
    }
}

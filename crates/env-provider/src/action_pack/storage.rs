use std::collections::BTreeSet;
use std::fs::{self};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use reqwest::header::HeaderName;
use semver::{Version, VersionReq};

use super::error::{ActionPackError, invalid_pack, storage_failed};
use super::model::{
    ActionBindingInfo, ActionDefinition, ActionKind, ActionPackInfo, ActionPackManifest,
    CliActionProfile,
};
use crate::personal_provider::{
    find_executable, is_kebab_identifier, probe_version, validate_executable_candidate,
    validate_text,
};

const SCHEMA_VERSION: u32 = 1;
const ACTION_PROTOCOL_VERSION: &str = "0.1.0";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_PACKS: usize = 100;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedCliAction {
    pub executable: PathBuf,
    pub cli_version: Version,
    pub profile: CliActionProfile,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedActionPack {
    pub manifest: ActionPackManifest,
    pub cli: Option<ResolvedCliAction>,
}

pub fn install(
    source: &Path,
    app_data: &Path,
    replace: bool,
) -> Result<ActionPackInfo, ActionPackError> {
    let source = if source.is_dir() {
        source.join("action.json")
    } else {
        source.to_path_buf()
    };
    let manifest = read_manifest(&source)?;
    validate_manifest(&manifest)?;
    let directory = packs_directory(app_data);
    fs::create_dir_all(&directory).map_err(|_| storage_failed())?;
    let destination = directory.join(format!("{}.json", manifest.id));
    if destination.exists() && !replace {
        return Err(ActionPackError::new(
            "ACTION_PACK_EXISTS",
            "같은 ID의 Action Pack이 이미 설치되어 있습니다.",
        ));
    }

    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| invalid_pack())?;
    let mut staging = tempfile::NamedTempFile::new_in(&directory).map_err(|_| storage_failed())?;
    staging.write_all(&bytes).map_err(|_| storage_failed())?;
    staging.as_file().sync_all().map_err(|_| storage_failed())?;
    if replace && destination.exists() {
        fs::remove_file(&destination).map_err(|_| storage_failed())?;
    }
    staging
        .persist(&destination)
        .map_err(|_| storage_failed())?;
    Ok(pack_info(&manifest, None))
}

pub fn remove(id: &str, app_data: &Path) -> Result<(), ActionPackError> {
    validate_id(id)?;
    let path = packs_directory(app_data).join(format!("{id}.json"));
    if !path.exists() {
        return Err(ActionPackError::new(
            "ACTION_PACK_NOT_FOUND",
            "설치된 Action Pack을 찾지 못했습니다.",
        ));
    }
    reject_symlink(&path)?;
    fs::remove_file(path).map_err(|_| storage_failed())
}

pub fn list(root: &Path, app_data: &Path) -> Vec<ActionPackInfo> {
    let Ok(entries) = fs::read_dir(packs_directory(app_data)) else {
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

pub(crate) fn resolve(
    id: &str,
    root: &Path,
    app_data: &Path,
) -> Result<ResolvedActionPack, ActionPackError> {
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
    manifest: ActionPackManifest,
    root: &Path,
) -> Result<ResolvedActionPack, ActionPackError> {
    let cli = match &manifest.action {
        ActionDefinition::Cli {
            executable_candidates,
            version_args,
            profiles,
            ..
        } => {
            let executable = executable_candidates
                .iter()
                .find_map(|candidate| find_executable(candidate, root))
                .ok_or(ActionPackError::new(
                    "ACTION_CLI_NOT_FOUND",
                    "Action Pack에 필요한 CLI를 찾지 못했습니다.",
                ))?;
            let cli_version = probe_version(&executable, version_args).map_err(|_| {
                ActionPackError::new(
                    "ACTION_CLI_UNSUPPORTED",
                    "Action CLI 버전을 안전하게 확인하지 못했습니다.",
                )
            })?;
            let profile = profiles
                .iter()
                .find(|profile| {
                    VersionReq::parse(&profile.version_requirement)
                        .is_ok_and(|requirement| requirement.matches(&cli_version))
                })
                .cloned()
                .ok_or(ActionPackError::new(
                    "ACTION_CLI_UNSUPPORTED",
                    "설치된 CLI 버전과 호환되는 Action Profile이 없습니다.",
                ))?;
            Some(ResolvedCliAction {
                executable,
                cli_version,
                profile,
            })
        }
        ActionDefinition::Http { .. } => None,
    };
    Ok(ResolvedActionPack { manifest, cli })
}

fn read_manifest(path: &Path) -> Result<ActionPackManifest, ActionPackError> {
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

fn validate_manifest(manifest: &ActionPackManifest) -> Result<(), ActionPackError> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.action_protocol_version != ACTION_PROTOCOL_VERSION
        || Version::parse(&manifest.pack_version).is_err()
    {
        return Err(invalid_pack());
    }
    validate_id(&manifest.id)?;
    map_provider_validation(validate_text(&manifest.display_name, 1, 64))?;
    map_provider_validation(validate_text(&manifest.description, 0, 240))?;
    match &manifest.action {
        ActionDefinition::Cli {
            executable_candidates,
            version_args,
            profiles,
            secret_binding,
            result_policy,
            timeout_seconds,
            ..
        } => {
            if executable_candidates.is_empty()
                || executable_candidates.len() > 8
                || version_args.is_empty()
                || version_args.len() > 8
                || profiles.is_empty()
                || profiles.len() > 8
                || !is_binding_id(secret_binding)
                || !result_policy.success
                || !valid_timeout(*timeout_seconds)
            {
                return Err(invalid_pack());
            }
            for candidate in executable_candidates {
                map_provider_validation(validate_executable_candidate(candidate))?;
            }
            for argument in version_args {
                validate_literal(argument, 128)?;
            }
            let mut profile_ids = BTreeSet::new();
            for profile in profiles {
                if !is_kebab_identifier(&profile.id)
                    || !profile_ids.insert(profile.id.as_str())
                    || VersionReq::parse(&profile.version_requirement).is_err()
                    || profile.arguments.is_empty()
                    || profile.arguments.len() > 32
                {
                    return Err(invalid_pack());
                }
                let mut variable_name_slots = 0;
                for argument in &profile.arguments {
                    map_provider_validation(validate_text(argument, 1, 256))?;
                    variable_name_slots += validate_argument_template(argument)?;
                }
                if variable_name_slots != 1 {
                    return Err(invalid_pack());
                }
            }
        }
        ActionDefinition::Http {
            url,
            secret_bindings,
            result_policy,
            timeout_seconds,
            ..
        } => {
            validate_http_url(url)?;
            if secret_bindings.is_empty()
                || secret_bindings.len() > 16
                || result_policy.body
                || !valid_timeout(*timeout_seconds)
                || result_policy
                    .success_status_codes
                    .iter()
                    .any(|status| !(100..=599).contains(status))
                || result_policy
                    .success_status_codes
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != result_policy.success_status_codes.len()
            {
                return Err(invalid_pack());
            }
            for (id, binding) in secret_bindings {
                if !is_binding_id(id) {
                    return Err(invalid_pack());
                }
                let header = binding.name.as_deref().unwrap_or(id);
                HeaderName::from_bytes(header.as_bytes()).map_err(|_| invalid_pack())?;
                validate_secret_format(&binding.format)?;
            }
        }
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ActionPackError> {
    let valid = id.len() <= 96
        && id.starts_with("local.")
        && id.split('.').count() >= 3
        && id.split('.').all(is_kebab_identifier);
    if valid { Ok(()) } else { Err(invalid_pack()) }
}

fn is_binding_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_literal(value: &str, max: usize) -> Result<(), ActionPackError> {
    map_provider_validation(validate_text(value, 1, max))?;
    if value.contains(['{', '}']) {
        return Err(invalid_pack());
    }
    Ok(())
}

fn validate_argument_template(value: &str) -> Result<usize, ActionPackError> {
    let rendered = value.replace("{variableName}", "");
    if rendered.contains(['{', '}']) {
        return Err(invalid_pack());
    }
    Ok(value.matches("{variableName}").count())
}

fn validate_secret_format(format: &str) -> Result<(), ActionPackError> {
    map_provider_validation(validate_text(format, 7, 512))?;
    if format.matches("{value}").count() != 1 || format.replace("{value}", "").contains(['{', '}'])
    {
        return Err(invalid_pack());
    }
    Ok(())
}

fn validate_http_url(value: &str) -> Result<(), ActionPackError> {
    if value.len() > 2048 || value.contains(['{', '}']) {
        return Err(invalid_pack());
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid_pack())?;
    let host = url.host_str().ok_or_else(invalid_pack)?;
    let secure = url.scheme() == "https";
    let local_http =
        url.scheme() == "http" && matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]");
    if (!secure && !local_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_pack());
    }
    Ok(())
}

fn valid_timeout(timeout_seconds: u64) -> bool {
    (1..=120).contains(&timeout_seconds)
}

pub(crate) fn pack_info(
    manifest: &ActionPackManifest,
    resolved: Option<&ResolvedActionPack>,
) -> ActionPackInfo {
    let (kind, bindings, target, cli_version, profile_id, available) = match &manifest.action {
        ActionDefinition::Cli {
            executable_candidates,
            secret_binding,
            ..
        } => {
            let cli = resolved.and_then(|value| value.cli.as_ref());
            (
                ActionKind::Cli,
                vec![ActionBindingInfo {
                    id: secret_binding.clone(),
                    destination: "stdin".to_owned(),
                }],
                executable_candidates.first().cloned().unwrap_or_default(),
                cli.map(|value| value.cli_version.to_string()),
                cli.map(|value| value.profile.id.clone()),
                cli.is_some(),
            )
        }
        ActionDefinition::Http {
            url,
            secret_bindings,
            ..
        } => (
            ActionKind::Http,
            secret_bindings
                .iter()
                .map(|(id, binding)| ActionBindingInfo {
                    id: id.clone(),
                    destination: binding.name.clone().unwrap_or_else(|| id.clone()),
                })
                .collect(),
            url.clone(),
            None,
            None,
            true,
        ),
    };
    ActionPackInfo {
        id: manifest.id.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest.description.clone(),
        pack_version: manifest.pack_version.clone(),
        kind,
        available,
        bindings,
        target,
        cli_version,
        profile_id,
    }
}

fn packs_directory(app_data: &Path) -> PathBuf {
    app_data.join("action-packs").join("personal")
}

fn reject_symlink(path: &Path) -> Result<(), ActionPackError> {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(invalid_pack());
    }
    Ok(())
}

fn map_provider_validation<T>(
    result: Result<T, crate::provider_push::ProviderPushError>,
) -> Result<T, ActionPackError> {
    result.map_err(|_| invalid_pack())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::action_pack::{
        CliResultPolicy, CliSecretTransport, HttpActionMethod, HttpResultPolicy, HttpSecretBinding,
        HttpSecretSource,
    };

    fn http_manifest(url: &str) -> ActionPackManifest {
        ActionPackManifest {
            schema_version: 1,
            id: "local.example.health-check".to_owned(),
            display_name: "Example health check".to_owned(),
            description: "Synthetic HTTP action".to_owned(),
            pack_version: "1.0.0".to_owned(),
            action_protocol_version: "0.1.0".to_owned(),
            action: ActionDefinition::Http {
                method: HttpActionMethod::Get,
                url: url.to_owned(),
                secret_bindings: BTreeMap::from([(
                    "Authorization".to_owned(),
                    HttpSecretBinding {
                        source: HttpSecretSource::Header,
                        name: None,
                        format: "Bearer {value}".to_owned(),
                    },
                )]),
                result_policy: HttpResultPolicy {
                    status: true,
                    duration: true,
                    body: false,
                    success_status_codes: vec![200],
                },
                timeout_seconds: 10,
            },
        }
    }

    #[test]
    fn accepts_bounded_http_and_cli_manifests() {
        validate_manifest(&http_manifest("https://api.example.com/health")).expect("HTTP pack");

        let mut cli = http_manifest("https://api.example.com/health");
        cli.id = "local.example.cli-upload".to_owned();
        cli.action = ActionDefinition::Cli {
            executable_candidates: vec!["example-cli".to_owned()],
            version_args: vec!["--version".to_owned()],
            profiles: vec![CliActionProfile {
                id: "example-v2".to_owned(),
                version_requirement: ">=2,<4".to_owned(),
                arguments: vec![
                    "secret".to_owned(),
                    "set".to_owned(),
                    "{variableName}".to_owned(),
                ],
            }],
            secret_binding: "value".to_owned(),
            secret_transport: CliSecretTransport::Stdin,
            result_policy: CliResultPolicy {
                success: true,
                exit_code: true,
                duration: true,
            },
            timeout_seconds: 30,
        };
        validate_manifest(&cli).expect("CLI pack");
    }

    #[test]
    fn rejects_redirectable_or_value_returning_http_shapes() {
        for url in [
            "http://api.example.com/health",
            "https://user:secret@example.com/health",
            "https://api.example.com/health?token=fake",
        ] {
            assert_eq!(
                validate_manifest(&http_manifest(url))
                    .expect_err("unsafe URL")
                    .code,
                "ACTION_PACK_INVALID"
            );
        }
        let mut body = http_manifest("https://api.example.com/health");
        let ActionDefinition::Http { result_policy, .. } = &mut body.action else {
            unreachable!()
        };
        result_policy.body = true;
        assert!(validate_manifest(&body).is_err());
    }

    #[test]
    fn rejects_unknown_manifest_fields_and_unapproved_placeholders() {
        let mut value = serde_json::to_value(http_manifest("https://api.example.com/health"))
            .expect("manifest value");
        value
            .as_object_mut()
            .expect("manifest object")
            .insert("responseSelector".to_owned(), serde_json::json!("$.token"));
        assert!(serde_json::from_value::<ActionPackManifest>(value).is_err());

        let placeholder = http_manifest("https://api.example.com/{variableName}");
        assert!(validate_manifest(&placeholder).is_err());
        let mut placeholder = http_manifest("https://api.example.com/health");
        let ActionDefinition::Http {
            secret_bindings, ..
        } = &mut placeholder.action
        else {
            unreachable!()
        };
        secret_bindings
            .get_mut("Authorization")
            .expect("binding")
            .format = "Bearer {value} {other}".to_owned();
        assert!(validate_manifest(&placeholder).is_err());
    }

    #[test]
    fn install_is_local_and_requires_explicit_replace() {
        let source = tempfile::tempdir().expect("source");
        let app_data = tempfile::tempdir().expect("app data");
        let manifest = http_manifest("https://api.example.com/health");
        fs::write(
            source.path().join("action.json"),
            serde_json::to_vec(&manifest).expect("serialize"),
        )
        .expect("write");
        install(source.path(), app_data.path(), false).expect("install");
        assert_eq!(
            install(source.path(), app_data.path(), false)
                .expect_err("duplicate")
                .code,
            "ACTION_PACK_EXISTS"
        );
        install(source.path(), app_data.path(), true).expect("replace");
    }
}

use std::ffi::OsString;
use std::path::Path;

use env_core::ProviderValue;

use crate::personal_provider::ResolvedPersonalProvider;

use super::cli::run_with_stdin;
use super::error::{ProviderPushError, invalid_target};
use super::model::{ProviderEntryKind, ProviderPushRequest, ProviderPushResult};

pub(super) fn push(
    root: &Path,
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
    adapter: ResolvedPersonalProvider,
) -> Result<ProviderPushResult, ProviderPushError> {
    if request
        .selections
        .iter()
        .any(|selection| selection.kind != ProviderEntryKind::Secret)
    {
        return Err(ProviderPushError {
            code: "PERSONAL_PROVIDER_KIND_UNSUPPORTED",
            message: "Personal Provider Pack은 현재 Secret 전송만 지원합니다.",
        });
    }
    let target = match adapter.manifest.target.as_ref() {
        Some(_) => Some(
            validate_target(
                request
                    .personal_target
                    .as_deref()
                    .ok_or_else(invalid_target)?,
            )?
            .to_owned(),
        ),
        None => None,
    };

    let mut pushed_count = 0;
    let mut failed_keys = Vec::new();
    for value in &values {
        let args = render_args(&adapter, value.key(), target.as_deref())?;
        if run_with_stdin(&adapter.executable, root, &args, value.value().as_bytes()) {
            pushed_count += 1;
        } else {
            failed_keys.push(value.key().to_owned());
        }
    }
    Ok(ProviderPushResult {
        provider: adapter.manifest.id,
        pushed_count,
        failed_keys,
    })
}

fn render_args(
    adapter: &ResolvedPersonalProvider,
    key: &str,
    target: Option<&str>,
) -> Result<Vec<OsString>, ProviderPushError> {
    adapter
        .profile
        .push_args
        .iter()
        .map(|argument| {
            let mut rendered = argument.replace("{key}", key);
            if let (Some(spec), Some(target)) = (adapter.manifest.target.as_ref(), target) {
                rendered = rendered.replace(&format!("{{{}}}", spec.placeholder), target);
            }
            if rendered.contains('{') || rendered.contains('}') || rendered.len() > 512 {
                return Err(ProviderPushError {
                    code: "PERSONAL_PROVIDER_INVALID",
                    message: "Personal Provider Pack 인자를 안전하게 구성하지 못했습니다.",
                });
            }
            Ok(OsString::from(rendered))
        })
        .collect()
}

fn validate_target(value: &str) -> Result<&str, ProviderPushError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('-')
        || value.chars().any(char::is_control)
    {
        return Err(invalid_target());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::personal_provider::{
        PersonalCliProfile, PersonalCliSpec, PersonalProviderManifest, PersonalTargetSpec,
        PersonalValueTransport,
    };
    use semver::Version;

    use super::*;

    fn resolved() -> ResolvedPersonalProvider {
        ResolvedPersonalProvider {
            manifest: PersonalProviderManifest {
                schema_version: 1,
                id: "local.example.deploy".to_owned(),
                display_name: "Example".to_owned(),
                description: String::new(),
                version: "1.0.0".to_owned(),
                provider_protocol_version: "0.1.0".to_owned(),
                value_transport: PersonalValueTransport::Stdin,
                target: Some(PersonalTargetSpec {
                    label: "App".to_owned(),
                    placeholder: "target".to_owned(),
                }),
                cli: PersonalCliSpec {
                    executable_candidates: vec!["fake-provider".to_owned()],
                    version_args: vec!["--version".to_owned()],
                    profiles: Vec::new(),
                },
            },
            executable: "fake-provider".into(),
            cli_version: Version::new(1, 2, 3),
            profile: PersonalCliProfile {
                id: "fake-v1".to_owned(),
                version_requirement: ">=1".to_owned(),
                push_args: vec![
                    "secret".to_owned(),
                    "{key}".to_owned(),
                    "--app={target}".to_owned(),
                ],
            },
        }
    }

    #[test]
    fn renders_metadata_without_a_value_slot() {
        let args = render_args(&resolved(), "FAKE_API_KEY", Some("demo-app")).expect("args");
        assert_eq!(
            args,
            ["secret", "FAKE_API_KEY", "--app=demo-app"].map(OsString::from)
        );
    }

    #[test]
    fn target_cannot_be_an_option() {
        assert!(validate_target("--danger").is_err());
    }
}

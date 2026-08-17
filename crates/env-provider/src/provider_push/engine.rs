use std::path::Path;

use env_core::ProjectService;

use crate::personal_provider::{self, ResolvedPersonalProvider};
use crate::provider_adapter::{self, AdapterStatus, ResolvedAdapter};
use crate::runtime_target::{self, REMOTE_RUNTIME_PROVIDER_ID};

use super::aws;
use super::cloudflare;
use super::error::{ProviderPushError, invalid_target};
use super::github;
use super::model::{
    AWS_SECRETS_MANAGER_ID, AWS_SSM_PARAMETER_STORE_ID, CLOUDFLARE_WORKERS_ID,
    DeploymentProviderSource, DeploymentProviderStatus, GITHUB_ACTIONS_ID, OfficialProviderId,
    ProviderCompareRequest, ProviderCompareResult, ProviderComparisonItem, ProviderComparisonState,
    ProviderPushRequest, ProviderPushResult,
};
use super::personal;

pub fn list(root: &Path, app_data: &Path) -> Vec<DeploymentProviderStatus> {
    let github = provider_adapter::resolve(OfficialProviderId::GithubActions, root, app_data);
    let cloudflare =
        provider_adapter::resolve(OfficialProviderId::CloudflareWorkers, root, app_data);
    let mut providers = vec![
        DeploymentProviderStatus {
            id: GITHUB_ACTIONS_ID.to_owned(),
            name: "GitHub Actions".to_owned(),
            available: github.is_ok(),
            detail: if github.is_ok() {
                "GitHub CLI ready".to_owned()
            } else {
                "GitHub CLI is missing or unavailable".to_owned()
            },
            source: DeploymentProviderSource::Official,
            version: None,
            target_label: None,
            adapter: github.as_ref().ok().map(AdapterStatus::from),
        },
        DeploymentProviderStatus {
            id: AWS_SECRETS_MANAGER_ID.to_owned(),
            name: "AWS Secrets Manager".to_owned(),
            available: true,
            detail: "Built-in AWS SDK · standard credential chain".to_owned(),
            source: DeploymentProviderSource::Official,
            version: Some("1.1.0".to_owned()),
            target_label: Some("Secret path prefix".to_owned()),
            adapter: None,
        },
        DeploymentProviderStatus {
            id: AWS_SSM_PARAMETER_STORE_ID.to_owned(),
            name: "AWS SSM Parameter Store".to_owned(),
            available: true,
            detail: "Built-in AWS SDK · SecureString".to_owned(),
            source: DeploymentProviderSource::Official,
            version: Some("1.1.0".to_owned()),
            target_label: Some("Parameter path prefix".to_owned()),
            adapter: None,
        },
        DeploymentProviderStatus {
            id: CLOUDFLARE_WORKERS_ID.to_owned(),
            name: "Cloudflare Workers".to_owned(),
            available: cloudflare.is_ok(),
            detail: if cloudflare.is_ok() {
                "Compatible Wrangler ready".to_owned()
            } else {
                "Wrangler is missing, unavailable, or unsupported".to_owned()
            },
            source: DeploymentProviderSource::Official,
            version: None,
            target_label: None,
            adapter: cloudflare.as_ref().ok().map(AdapterStatus::from),
        },
        DeploymentProviderStatus {
            id: REMOTE_RUNTIME_PROVIDER_ID.to_owned(),
            name: "Remote Runtime".to_owned(),
            available: runtime_target::list(root).is_ok_and(|targets| !targets.is_empty()),
            detail: "Encrypted fixed-verifier comparison".to_owned(),
            source: DeploymentProviderSource::Official,
            version: Some("1.0.0".to_owned()),
            target_label: Some("Runtime target".to_owned()),
            adapter: None,
        },
    ];
    providers.extend(
        personal_provider::list(root, app_data)
            .into_iter()
            .map(|pack| DeploymentProviderStatus {
                id: pack.id,
                name: pack.display_name,
                available: pack.available,
                detail: pack.description,
                source: DeploymentProviderSource::Personal,
                version: Some(pack.version),
                target_label: pack.target_label,
                adapter: match (pack.cli_version, pack.profile_id) {
                    (Some(cli_version), Some(profile_id)) => Some(AdapterStatus {
                        cli_version,
                        profile_id,
                        adapter_version: "personal".to_owned(),
                        adapter_source: crate::provider_adapter::AdapterSource::Personal,
                    }),
                    _ => None,
                },
            }),
    );
    providers
}

pub fn push(
    service: &ProjectService,
    app_data: &Path,
    request: ProviderPushRequest,
) -> Result<ProviderPushResult, ProviderPushError> {
    let prepared = prepare(service, app_data, &request)?;
    let keys = request
        .selections
        .iter()
        .map(|selection| selection.key.clone())
        .collect::<Vec<_>>();
    let values = service
        .provider_values(&request.file, &keys)
        .map_err(ProviderPushError::from)?;
    match prepared {
        PreparedProvider::Github(adapter) => github::push(service.root(), request, values, adapter),
        PreparedProvider::Cloudflare(adapter) => {
            cloudflare::push(service.root(), request, values, adapter)
        }
        PreparedProvider::Personal(adapter) => {
            personal::push(service.root(), request, values, adapter)
        }
        PreparedProvider::Aws(adapter) => aws::push(values, adapter),
    }
}

pub fn compare(
    service: &ProjectService,
    request: ProviderCompareRequest,
) -> Result<ProviderCompareResult, ProviderPushError> {
    let values = service
        .provider_values(&request.file, &request.keys)
        .map_err(ProviderPushError::from)?;
    match request.provider.as_str() {
        AWS_SECRETS_MANAGER_ID => {
            aws::compare(values, OfficialProviderId::AwsSecretsManager, &request)
        }
        AWS_SSM_PARAMETER_STORE_ID => {
            aws::compare(values, OfficialProviderId::AwsSsmParameterStore, &request)
        }
        REMOTE_RUNTIME_PROVIDER_ID => {
            let target_id = request
                .runtime_target_id
                .as_deref()
                .ok_or(ProviderPushError {
                    code: "REMOTE_TARGET_REQUIRED",
                    message: "확인할 원격 Runtime 대상을 선택해주세요.",
                })?;
            runtime_target::compare(service.root(), target_id, &request.file, values)
        }
        GITHUB_ACTIONS_ID | CLOUDFLARE_WORKERS_ID => Ok(ProviderCompareResult {
            provider: request.provider,
            target: "unreadable-secret-store".to_owned(),
            items: request
                .keys
                .into_iter()
                .map(|key| ProviderComparisonItem {
                    remote_name: key.clone(),
                    key,
                    state: ProviderComparisonState::Unverifiable,
                    result_code: Some("REMOTE_VALUE_UNREADABLE".to_owned()),
                })
                .collect(),
        }),
        _ => Err(ProviderPushError {
            code: "PROVIDER_COMPARE_UNSUPPORTED",
            message: "이 Provider는 값 일치 확인을 지원하지 않습니다.",
        }),
    }
}

enum PreparedProvider {
    Github(ResolvedAdapter),
    Cloudflare(ResolvedAdapter),
    Personal(ResolvedPersonalProvider),
    Aws(aws::PreparedAwsProvider),
}

fn prepare(
    service: &ProjectService,
    app_data: &Path,
    request: &ProviderPushRequest,
) -> Result<PreparedProvider, ProviderPushError> {
    match request.provider.as_str() {
        GITHUB_ACTIONS_ID => {
            provider_adapter::resolve(OfficialProviderId::GithubActions, service.root(), app_data)
                .map(PreparedProvider::Github)
        }
        CLOUDFLARE_WORKERS_ID => {
            let worker = request.worker.as_deref().ok_or_else(invalid_target)?;
            let (access, adapter) = cloudflare::preflight::inspect_with_adapter(
                service.root(),
                app_data,
                &request.file,
                worker,
                request.cloudflare_environment.as_deref(),
            )?;
            cloudflare::preflight::ensure_access(&access)?;
            Ok(PreparedProvider::Cloudflare(adapter))
        }
        AWS_SECRETS_MANAGER_ID => {
            aws::prepare(OfficialProviderId::AwsSecretsManager, request).map(PreparedProvider::Aws)
        }
        AWS_SSM_PARAMETER_STORE_ID => {
            aws::prepare(OfficialProviderId::AwsSsmParameterStore, request)
                .map(PreparedProvider::Aws)
        }
        personal_id if personal_id.starts_with("local.") => {
            personal_provider::resolve(personal_id, service.root(), app_data)
                .map(PreparedProvider::Personal)
        }
        _ => Err(ProviderPushError {
            code: "PROVIDER_UNSUPPORTED",
            message: "지원하지 않는 Provider입니다.",
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn unreadable_secret_stores_report_unverifiable_without_value_access() {
        let project = tempfile::tempdir().expect("synthetic project");
        fs::write(
            project.path().join(".env.local"),
            "DEMO_TOKEN=fake_provider_compare_canary\n",
        )
        .expect("synthetic env fixture");
        let service = ProjectService::open(project.path()).expect("open synthetic project");
        let result = compare(
            &service,
            ProviderCompareRequest {
                provider: GITHUB_ACTIONS_ID.to_owned(),
                file: ".env.local".to_owned(),
                keys: vec!["DEMO_TOKEN".to_owned()],
                aws_profile: None,
                aws_region: None,
                aws_path_prefix: None,
                runtime_target_id: None,
            },
        )
        .expect("redacted comparison result");

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].state, ProviderComparisonState::Unverifiable);
        let serialized = serde_json::to_string(&result).expect("serialize redacted result");
        assert!(!serialized.contains("fake_provider_compare_canary"));
    }
}

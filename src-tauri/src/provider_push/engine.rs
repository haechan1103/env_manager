use std::path::Path;

use env_core::ProjectService;

use crate::provider_adapter::{self, AdapterStatus, ResolvedAdapter};

use super::cloudflare;
use super::error::{ProviderPushError, invalid_target};
use super::github;
use super::model::{
    DeploymentProviderId, DeploymentProviderStatus, ProviderPushRequest, ProviderPushResult,
};

pub fn list(root: &Path, app_data: &Path) -> Vec<DeploymentProviderStatus> {
    let github = provider_adapter::resolve(DeploymentProviderId::GithubActions, root, app_data);
    let cloudflare =
        provider_adapter::resolve(DeploymentProviderId::CloudflareWorkers, root, app_data);
    vec![
        DeploymentProviderStatus {
            id: DeploymentProviderId::GithubActions,
            name: "GitHub Actions",
            available: github.is_ok(),
            detail: if github.is_ok() {
                "GitHub CLI ready"
            } else {
                "GitHub CLI is missing or unavailable"
            },
            adapter: github.as_ref().ok().map(AdapterStatus::from),
        },
        DeploymentProviderStatus {
            id: DeploymentProviderId::CloudflareWorkers,
            name: "Cloudflare Workers",
            available: cloudflare.is_ok(),
            detail: if cloudflare.is_ok() {
                "Compatible Wrangler ready"
            } else {
                "Wrangler is missing, unavailable, or unsupported"
            },
            adapter: cloudflare.as_ref().ok().map(AdapterStatus::from),
        },
    ]
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
    }
}

enum PreparedProvider {
    Github(ResolvedAdapter),
    Cloudflare(ResolvedAdapter),
}

fn prepare(
    service: &ProjectService,
    app_data: &Path,
    request: &ProviderPushRequest,
) -> Result<PreparedProvider, ProviderPushError> {
    match request.provider {
        DeploymentProviderId::GithubActions => provider_adapter::resolve(
            DeploymentProviderId::GithubActions,
            service.root(),
            app_data,
        )
        .map(PreparedProvider::Github),
        DeploymentProviderId::CloudflareWorkers => {
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
    }
}

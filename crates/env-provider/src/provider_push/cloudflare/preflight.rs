use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use serde::Deserialize;

use crate::provider_adapter::{self, AdapterStatus, AdapterStrategy, ResolvedAdapter};

use super::super::cli::provider_command;
use super::super::error::ProviderPushError;
use super::super::model::{
    CloudflareAccessContext, CloudflareAccountState, CloudflareAuthState, CloudflareTargetState,
    OfficialProviderId,
};
use super::super::validation::{optional_target, validate_simple_target};
use super::config::detect_target;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudflareWhoami {
    logged_in: bool,
    auth_type: Option<String>,
    accounts: Option<Vec<CloudflareAccount>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CloudflareAccount {
    id: String,
    name: String,
}

pub fn inspect(
    root: &Path,
    app_data: &Path,
    source_file: &str,
    worker: &str,
    environment: Option<&str>,
) -> Result<CloudflareAccessContext, ProviderPushError> {
    let (context, _) = inspect_with_adapter(root, app_data, source_file, worker, environment)?;
    Ok(context)
}

pub(in crate::provider_push) fn inspect_with_adapter(
    root: &Path,
    app_data: &Path,
    source_file: &str,
    worker: &str,
    environment: Option<&str>,
) -> Result<(CloudflareAccessContext, ResolvedAdapter), ProviderPushError> {
    validate_simple_target(worker)?;
    let environment = optional_target(environment)?;
    let adapter = provider_adapter::resolve(OfficialProviderId::CloudflareWorkers, root, app_data)?;
    if adapter.strategy != AdapterStrategy::WranglerSecretBulkV1 {
        return Err(ProviderPushError {
            code: "PROVIDER_ADAPTER_INVALID",
            message: "Cloudflare Adapter 전략이 올바르지 않습니다.",
        });
    }

    let target = detect_target(root, source_file)?;
    let configured_account_id = environment
        .and_then(|name| target.environment_account_ids.get(name))
        .or(target.account_id.as_ref())
        .cloned();
    let config = target.config_path.as_deref().map(|path| root.join(path));
    let whoami = run_whoami(&adapter.executable, root, config.as_deref(), environment);
    let Some(whoami) = whoami else {
        return Ok((
            unavailable_context(configured_account_id, &adapter),
            adapter,
        ));
    };
    if !whoami.logged_in {
        return Ok((
            unauthenticated_context(configured_account_id, &adapter),
            adapter,
        ));
    }

    let accounts = whoami.accounts.unwrap_or_default();
    let matched_account = configured_account_id
        .as_deref()
        .and_then(|id| accounts.iter().find(|account| account.id == id))
        .cloned()
        .or_else(|| {
            (configured_account_id.is_none() && accounts.len() == 1).then(|| accounts[0].clone())
        });
    let account_state = classify_account(configured_account_id.as_deref(), &accounts);
    let target_state = if account_state == CloudflareAccountState::Mismatch {
        CloudflareTargetState::Unchecked
    } else if target_accessible(
        &adapter.executable,
        root,
        config.as_deref(),
        worker,
        environment,
    ) {
        CloudflareTargetState::Accessible
    } else {
        CloudflareTargetState::Unavailable
    };
    let account_id = configured_account_id
        .or_else(|| matched_account.as_ref().map(|account| account.id.clone()));
    let account_name = matched_account.map(|account| account.name);
    Ok((
        CloudflareAccessContext {
            auth_state: CloudflareAuthState::Authenticated,
            auth_type: whoami.auth_type,
            account_state,
            account_id,
            account_name,
            account_count: accounts.len(),
            target_state,
            adapter: AdapterStatus::from(&adapter),
        },
        adapter,
    ))
}

pub(in crate::provider_push) fn ensure_access(
    access: &CloudflareAccessContext,
) -> Result<(), ProviderPushError> {
    match access.auth_state {
        CloudflareAuthState::NotAuthenticated => Err(ProviderPushError {
            code: "CLOUDFLARE_NOT_AUTHENTICATED",
            message: "Wrangler 로그인이 필요합니다.",
        }),
        CloudflareAuthState::Unavailable => Err(ProviderPushError {
            code: "CLOUDFLARE_AUTH_CHECK_FAILED",
            message: "Cloudflare 로그인 상태를 확인하지 못했습니다.",
        }),
        CloudflareAuthState::Authenticated
            if access.account_state == CloudflareAccountState::Mismatch =>
        {
            Err(ProviderPushError {
                code: "CLOUDFLARE_ACCOUNT_MISMATCH",
                message: "Wrangler 설정 계정과 현재 로그인 계정이 일치하지 않습니다.",
            })
        }
        CloudflareAuthState::Authenticated
            if access.target_state != CloudflareTargetState::Accessible =>
        {
            Err(ProviderPushError {
                code: "CLOUDFLARE_TARGET_UNAVAILABLE",
                message: "현재 계정으로 Worker를 확인할 수 없습니다.",
            })
        }
        CloudflareAuthState::Authenticated => Ok(()),
    }
}

pub(super) fn append_context(
    args: &mut Vec<OsString>,
    config: Option<&Path>,
    environment: Option<&str>,
) {
    if let Some(config) = config {
        args.push(OsString::from("--config"));
        args.push(config.as_os_str().to_owned());
    }
    if let Some(environment) = environment {
        args.push(OsString::from("--env"));
        args.push(OsString::from(environment));
    }
}

fn unavailable_context(
    account_id: Option<String>,
    adapter: &ResolvedAdapter,
) -> CloudflareAccessContext {
    CloudflareAccessContext {
        auth_state: CloudflareAuthState::Unavailable,
        auth_type: None,
        account_state: CloudflareAccountState::Unchecked,
        account_id,
        account_name: None,
        account_count: 0,
        target_state: CloudflareTargetState::Unchecked,
        adapter: AdapterStatus::from(adapter),
    }
}

fn unauthenticated_context(
    account_id: Option<String>,
    adapter: &ResolvedAdapter,
) -> CloudflareAccessContext {
    CloudflareAccessContext {
        auth_state: CloudflareAuthState::NotAuthenticated,
        auth_type: None,
        account_state: CloudflareAccountState::Unchecked,
        account_id,
        account_name: None,
        account_count: 0,
        target_state: CloudflareTargetState::Unchecked,
        adapter: AdapterStatus::from(adapter),
    }
}

fn classify_account(
    configured_account_id: Option<&str>,
    accounts: &[CloudflareAccount],
) -> CloudflareAccountState {
    match configured_account_id {
        Some(id) if accounts.iter().any(|account| account.id == id) => {
            CloudflareAccountState::Matched
        }
        Some(_) => CloudflareAccountState::Mismatch,
        None if accounts.len() > 1 => CloudflareAccountState::Ambiguous,
        None if accounts.len() == 1 => CloudflareAccountState::Matched,
        None => CloudflareAccountState::Unconfigured,
    }
}

fn run_whoami(
    executable: &Path,
    root: &Path,
    config: Option<&Path>,
    environment: Option<&str>,
) -> Option<CloudflareWhoami> {
    let mut args = vec![OsString::from("whoami"), OsString::from("--json")];
    append_context(&mut args, config, environment);
    let output = provider_command(executable, &args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if output.stdout.len() > 2 * 1024 * 1024 || output.stderr.len() > 2 * 1024 * 1024 {
        return None;
    }
    parse_whoami(&output.stdout, &output.stderr)
}

fn parse_whoami(stdout: &[u8], stderr: &[u8]) -> Option<CloudflareWhoami> {
    serde_json::from_slice(stdout)
        .ok()
        .or_else(|| serde_json::from_slice(stderr).ok())
}

fn target_accessible(
    executable: &Path,
    root: &Path,
    config: Option<&Path>,
    worker: &str,
    environment: Option<&str>,
) -> bool {
    let mut args = vec![
        OsString::from("secret"),
        OsString::from("list"),
        OsString::from("--name"),
        OsString::from(worker),
        OsString::from("--format"),
        OsString::from("json"),
    ];
    append_context(&mut args, config, environment);
    provider_command(executable, &args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
#[path = "preflight_tests.rs"]
mod tests;

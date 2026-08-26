mod config;
pub(super) mod preflight;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use env_core::ProviderValue;
use zeroize::Zeroizing;

use crate::provider_adapter::{AdapterStrategy, ResolvedAdapter};

use super::cli::run_with_stdin;
use super::error::{ProviderPushError, invalid_request, invalid_target};
use super::model::{
    CLOUDFLARE_WORKERS_ID, ProviderEntryKind, ProviderPushRequest, ProviderPushResult,
};
use super::validation::{optional_target, validate_simple_target};

pub use config::detect_target;
pub use preflight::inspect;

pub(super) fn push(
    root: &Path,
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
    adapter: ResolvedAdapter,
) -> Result<ProviderPushResult, ProviderPushError> {
    if request
        .selections
        .iter()
        .any(|selection| selection.kind != ProviderEntryKind::Secret)
    {
        return Err(invalid_request(
            "Cloudflare Workers에는 현재 Secret만 전송할 수 있습니다.",
        ));
    }
    if adapter.strategy != AdapterStrategy::WranglerSecretBulkV1 {
        return Err(ProviderPushError {
            code: "PROVIDER_ADAPTER_INVALID",
            message: "Cloudflare Adapter 전략이 올바르지 않습니다.",
        });
    }
    let worker = request.worker.as_deref().ok_or_else(invalid_target)?;
    validate_simple_target(worker)?;
    let environment = optional_target(request.cloudflare_environment.as_deref())?;
    let mut args = vec![
        OsString::from("secret"),
        OsString::from("bulk"),
        OsString::from("--name"),
        OsString::from(worker),
    ];
    let target = detect_target(root, &request.file)?;
    let config = target.config_path.as_deref().map(|path| root.join(path));
    preflight::append_context(&mut args, config.as_deref(), environment);

    let borrowed = values
        .iter()
        .map(|value| (value.key(), value.value()))
        .collect::<BTreeMap<_, _>>();
    let mut stdin = Zeroizing::new(Vec::new());
    serde_json::to_writer(&mut *stdin, &borrowed).map_err(|_| ProviderPushError {
        code: "PROVIDER_PAYLOAD_FAILED",
        message: "Cloudflare 전송 데이터를 준비하지 못했습니다.",
    })?;
    if !run_with_stdin(&adapter.executable, root, &args, &stdin) {
        return Err(ProviderPushError {
            code: "PROVIDER_PUSH_FAILED",
            message: "Cloudflare 전송에 실패했습니다. Wrangler 로그인과 대상을 확인해주세요.",
        });
    }
    Ok(ProviderPushResult {
        provider: CLOUDFLARE_WORKERS_ID.to_owned(),
        pushed_count: values.len(),
        failed_keys: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use env_core::ProjectService;

    use super::*;

    #[test]
    fn cloudflare_payload_is_json_without_debuggable_value_type() {
        let project = tempfile::tempdir().expect("project");
        std::fs::write(
            project.path().join(".env"),
            "API_KEY=fake_secret_canary\nPORT=fake_8787\n",
        )
        .expect("fixture");
        let service = ProjectService::open(project.path()).expect("service");
        service.initialize().expect("initialize");
        let values = service
            .provider_values(".env", &["API_KEY".to_owned(), "PORT".to_owned()])
            .expect("values");
        let borrowed = values
            .iter()
            .map(|value| (value.key(), value.value()))
            .collect::<BTreeMap<_, _>>();
        let mut buffer = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *buffer, &borrowed).expect("serialize");
        assert_eq!(
            String::from_utf8_lossy(&buffer),
            r#"{"API_KEY":"fake_secret_canary","PORT":"fake_8787"}"#
        );
    }
}

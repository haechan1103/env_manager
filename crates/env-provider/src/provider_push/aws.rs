use std::sync::OnceLock;

use aws_config::{BehaviorVersion, Region, SdkConfig, retry::RetryConfig};
use aws_sdk_ssm::types::ParameterType;
use env_core::ProviderValue;
use serde::Serialize;
use zeroize::Zeroizing;

use super::error::{ProviderPushError, invalid_target};
use super::model::{
    AWS_SECRETS_MANAGER_ID, AWS_SSM_PARAMETER_STORE_ID, OfficialProviderId, ProviderCompareRequest,
    ProviderCompareResult, ProviderComparisonItem, ProviderComparisonState, ProviderPushRequest,
    ProviderPushResult,
};

const MAX_KMS_ALIASES: usize = 500;

pub(super) struct PreparedAwsProvider {
    pub provider: OfficialProviderId,
    pub config: SdkConfig,
    pub prefix: String,
    pub kms_key_id: Option<String>,
}

struct PreparedAwsComparison {
    provider: OfficialProviderId,
    config: SdkConfig,
    prefix: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsAccessContext {
    pub account_id: String,
    pub principal_arn: Option<String>,
    pub region: String,
    pub kms_aliases: Vec<String>,
    pub kms_aliases_available: bool,
}

pub fn inspect_access(
    profile: Option<&str>,
    region: Option<&str>,
) -> Result<AwsAccessContext, ProviderPushError> {
    runtime()?.block_on(inspect_access_async(profile, region))
}

pub(super) fn prepare(
    provider: OfficialProviderId,
    request: &ProviderPushRequest,
) -> Result<PreparedAwsProvider, ProviderPushError> {
    let profile = validate_optional_profile(request.aws_profile.as_deref())?;
    let region = validate_optional_region(request.aws_region.as_deref())?;
    let prefix = validate_prefix(request.aws_path_prefix.as_deref())?.to_owned();
    let kms_key_id =
        validate_optional_kms_key(request.aws_kms_key_id.as_deref())?.map(str::to_owned);
    let prepared = runtime()?.block_on(async {
        let (config, _) = load_and_verify(profile, region).await?;
        if let Some(key_id) = kms_key_id.as_deref() {
            let key = aws_sdk_kms::Client::new(&config)
                .describe_key()
                .key_id(key_id)
                .send()
                .await
                .map_err(|_| ProviderPushError {
                    code: "AWS_KMS_KEY_UNAVAILABLE",
                    message: "선택한 KMS 키를 현재 AWS 계정과 Region에서 확인하지 못했습니다.",
                })?;
            let symmetric_encrypt_key = key.key_metadata().is_some_and(|metadata| {
                metadata
                    .key_spec()
                    .is_some_and(|spec| spec.as_str() == "SYMMETRIC_DEFAULT")
                    && metadata
                        .key_usage()
                        .is_some_and(|usage| usage.as_str() == "ENCRYPT_DECRYPT")
            });
            if !symmetric_encrypt_key {
                return Err(ProviderPushError {
                    code: "AWS_KMS_KEY_UNSUPPORTED",
                    message: "Secrets Manager와 SSM에는 대칭형 암호화 KMS 키가 필요합니다.",
                });
            }
        }
        Ok::<PreparedAwsProvider, ProviderPushError>(PreparedAwsProvider {
            provider,
            config,
            prefix,
            kms_key_id,
        })
    })?;
    Ok(prepared)
}

pub(super) fn push(
    values: Vec<ProviderValue>,
    prepared: PreparedAwsProvider,
) -> Result<ProviderPushResult, ProviderPushError> {
    runtime()?.block_on(push_async(values, prepared))
}

pub(super) fn compare(
    values: Vec<ProviderValue>,
    provider: OfficialProviderId,
    request: &ProviderCompareRequest,
) -> Result<ProviderCompareResult, ProviderPushError> {
    let profile = validate_optional_profile(request.aws_profile.as_deref())?;
    let region = validate_optional_region(request.aws_region.as_deref())?;
    let prefix = validate_prefix(request.aws_path_prefix.as_deref())?.to_owned();
    let prepared = runtime()?.block_on(async {
        let (config, _) = load_and_verify(profile, region).await?;
        Ok::<PreparedAwsComparison, ProviderPushError>(PreparedAwsComparison {
            provider,
            config,
            prefix,
        })
    })?;
    runtime()?.block_on(compare_async(values, prepared))
}

async fn inspect_access_async(
    profile: Option<&str>,
    region: Option<&str>,
) -> Result<AwsAccessContext, ProviderPushError> {
    let profile = validate_optional_profile(profile)?;
    let region = validate_optional_region(region)?;
    let (config, identity) = load_and_verify(profile, region).await?;
    let resolved_region = config
        .region()
        .map(|value| value.as_ref().to_owned())
        .ok_or_else(aws_region_missing)?;
    let (kms_aliases, kms_aliases_available) = list_kms_aliases(&config).await;
    Ok(AwsAccessContext {
        account_id: identity.account().unwrap_or_default().to_owned(),
        principal_arn: identity.arn().map(str::to_owned),
        region: resolved_region,
        kms_aliases,
        kms_aliases_available,
    })
}

async fn load_and_verify(
    profile: Option<&str>,
    region: Option<&str>,
) -> Result<
    (
        SdkConfig,
        aws_sdk_sts::operation::get_caller_identity::GetCallerIdentityOutput,
    ),
    ProviderPushError,
> {
    let mut loader =
        aws_config::defaults(BehaviorVersion::latest()).retry_config(RetryConfig::disabled());
    if let Some(profile) = profile {
        loader = loader.profile_name(profile);
    }
    if let Some(region) = region {
        loader = loader.region(Region::new(region.to_owned()));
    }
    let config = loader.load().await;
    if config.region().is_none() {
        return Err(aws_region_missing());
    }
    let identity = aws_sdk_sts::Client::new(&config)
        .get_caller_identity()
        .send()
        .await
        .map_err(|_| ProviderPushError {
            code: "AWS_AUTH_UNAVAILABLE",
            message: "AWS 로그인을 확인하지 못했습니다. Profile, SSO 세션, 자격 증명과 권한을 확인해주세요.",
        })?;
    if identity.account().is_none_or(str::is_empty) {
        return Err(ProviderPushError {
            code: "AWS_AUTH_UNAVAILABLE",
            message: "현재 AWS 계정 정보를 확인하지 못했습니다.",
        });
    }
    Ok((config, identity))
}

async fn list_kms_aliases(config: &SdkConfig) -> (Vec<String>, bool) {
    let client = aws_sdk_kms::Client::new(config);
    let mut aliases = Vec::new();
    let mut marker: Option<String> = None;
    loop {
        let response = client
            .list_aliases()
            .limit(100)
            .set_marker(marker)
            .send()
            .await;
        let Ok(response) = response else {
            return (Vec::new(), false);
        };
        aliases.extend(
            response
                .aliases()
                .iter()
                .filter_map(|alias| alias.alias_name().map(str::to_owned)),
        );
        if aliases.len() >= MAX_KMS_ALIASES || !response.truncated() {
            break;
        }
        marker = response.next_marker().map(str::to_owned);
        if marker.is_none() {
            break;
        }
    }
    aliases.truncate(MAX_KMS_ALIASES);
    aliases.sort();
    aliases.dedup();
    (aliases, true)
}

async fn push_async(
    values: Vec<ProviderValue>,
    prepared: PreparedAwsProvider,
) -> Result<ProviderPushResult, ProviderPushError> {
    let mut pushed_count = 0;
    let mut failed_keys = Vec::new();
    match prepared.provider {
        OfficialProviderId::AwsSecretsManager => {
            let client = aws_sdk_secretsmanager::Client::new(&prepared.config);
            for value in &values {
                let name = remote_name(&prepared.prefix, value.key())?;
                if push_secret(
                    &client,
                    &name,
                    value.value(),
                    prepared.kms_key_id.as_deref(),
                )
                .await
                {
                    pushed_count += 1;
                } else {
                    failed_keys.push(value.key().to_owned());
                }
            }
        }
        OfficialProviderId::AwsSsmParameterStore => {
            let client = aws_sdk_ssm::Client::new(&prepared.config);
            for value in &values {
                let name = remote_name(&prepared.prefix, value.key())?;
                let sent = client
                    .put_parameter()
                    .name(name)
                    .value(value.value())
                    .r#type(ParameterType::SecureString)
                    .overwrite(true)
                    .set_key_id(prepared.kms_key_id.clone())
                    .send()
                    .await
                    .is_ok();
                if sent {
                    pushed_count += 1;
                } else {
                    failed_keys.push(value.key().to_owned());
                }
            }
        }
        _ => {
            return Err(ProviderPushError {
                code: "PROVIDER_ADAPTER_INVALID",
                message: "AWS Provider Adapter가 올바르지 않습니다.",
            });
        }
    }
    let provider_id = match prepared.provider {
        OfficialProviderId::AwsSecretsManager => AWS_SECRETS_MANAGER_ID,
        OfficialProviderId::AwsSsmParameterStore => AWS_SSM_PARAMETER_STORE_ID,
        _ => {
            return Err(ProviderPushError {
                code: "PROVIDER_ADAPTER_INVALID",
                message: "AWS Provider Adapter가 올바르지 않습니다.",
            });
        }
    };
    Ok(ProviderPushResult {
        provider: provider_id.to_owned(),
        pushed_count,
        failed_keys,
    })
}

async fn compare_async(
    values: Vec<ProviderValue>,
    prepared: PreparedAwsComparison,
) -> Result<ProviderCompareResult, ProviderPushError> {
    let region = prepared
        .config
        .region()
        .map(|value| value.as_ref().to_owned())
        .ok_or_else(aws_region_missing)?;
    let target = if prepared.prefix.is_empty() {
        region
    } else {
        format!("{region}/{}", prepared.prefix)
    };
    let mut items = Vec::with_capacity(values.len());
    match prepared.provider {
        OfficialProviderId::AwsSecretsManager => {
            let client = aws_sdk_secretsmanager::Client::new(&prepared.config);
            for local in &values {
                let name = remote_name(&prepared.prefix, local.key())?;
                let item = match client.get_secret_value().secret_id(&name).send().await {
                    Ok(output) => match output.secret_string.map(Zeroizing::new) {
                        Some(remote) => comparison_item(local, name, remote.as_str()),
                        None => comparison_error(local.key(), name, "REMOTE_VALUE_UNSUPPORTED"),
                    },
                    Err(error)
                        if error
                            .as_service_error()
                            .is_some_and(|service| service.is_resource_not_found_exception()) =>
                    {
                        comparison_unset(local.key(), name)
                    }
                    Err(_) => comparison_error(local.key(), name, "REMOTE_READ_FAILED"),
                };
                items.push(item);
            }
        }
        OfficialProviderId::AwsSsmParameterStore => {
            let client = aws_sdk_ssm::Client::new(&prepared.config);
            for local in &values {
                let name = remote_name(&prepared.prefix, local.key())?;
                let item = match client
                    .get_parameter()
                    .name(&name)
                    .with_decryption(true)
                    .send()
                    .await
                {
                    Ok(output) => {
                        match output
                            .parameter
                            .and_then(|parameter| parameter.value)
                            .map(Zeroizing::new)
                        {
                            Some(remote) => comparison_item(local, name, remote.as_str()),
                            None => comparison_unset(local.key(), name),
                        }
                    }
                    Err(error)
                        if error
                            .as_service_error()
                            .is_some_and(|service| service.is_parameter_not_found()) =>
                    {
                        comparison_unset(local.key(), name)
                    }
                    Err(_) => comparison_error(local.key(), name, "REMOTE_READ_FAILED"),
                };
                items.push(item);
            }
        }
        _ => {
            return Err(ProviderPushError {
                code: "PROVIDER_ADAPTER_INVALID",
                message: "AWS Provider Adapter가 올바르지 않습니다.",
            });
        }
    }
    let provider = match prepared.provider {
        OfficialProviderId::AwsSecretsManager => AWS_SECRETS_MANAGER_ID,
        OfficialProviderId::AwsSsmParameterStore => AWS_SSM_PARAMETER_STORE_ID,
        _ => {
            return Err(ProviderPushError {
                code: "PROVIDER_ADAPTER_INVALID",
                message: "AWS Provider Adapter가 올바르지 않습니다.",
            });
        }
    };
    Ok(ProviderCompareResult {
        provider: provider.to_owned(),
        target,
        items,
    })
}

fn comparison_item(
    local: &ProviderValue,
    remote_name: String,
    remote: &str,
) -> ProviderComparisonItem {
    ProviderComparisonItem {
        key: local.key().to_owned(),
        remote_name,
        state: if constant_time_equal(local.value().as_bytes(), remote.as_bytes()) {
            ProviderComparisonState::Same
        } else {
            ProviderComparisonState::Different
        },
        result_code: None,
    }
}

fn comparison_unset(key: &str, remote_name: String) -> ProviderComparisonItem {
    ProviderComparisonItem {
        key: key.to_owned(),
        remote_name,
        state: ProviderComparisonState::Unset,
        result_code: None,
    }
}

fn comparison_error(key: &str, remote_name: String, result_code: &str) -> ProviderComparisonItem {
    ProviderComparisonItem {
        key: key.to_owned(),
        remote_name,
        state: ProviderComparisonState::Error,
        result_code: Some(result_code.to_owned()),
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

async fn push_secret(
    client: &aws_sdk_secretsmanager::Client,
    name: &str,
    value: &str,
    kms_key_id: Option<&str>,
) -> bool {
    match client.describe_secret().secret_id(name).send().await {
        Ok(_) if kms_key_id.is_some() => client
            .update_secret()
            .secret_id(name)
            .secret_string(value)
            .set_kms_key_id(kms_key_id.map(str::to_owned))
            .send()
            .await
            .is_ok(),
        Ok(_) => client
            .put_secret_value()
            .secret_id(name)
            .secret_string(value)
            .send()
            .await
            .is_ok(),
        Err(error)
            if error
                .as_service_error()
                .is_some_and(|service| service.is_resource_not_found_exception()) =>
        {
            client
                .create_secret()
                .name(name)
                .secret_string(value)
                .set_kms_key_id(kms_key_id.map(str::to_owned))
                .send()
                .await
                .is_ok()
        }
        Err(_) => false,
    }
}

fn remote_name(prefix: &str, key: &str) -> Result<String, ProviderPushError> {
    let name = if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{}/{}", prefix.trim_end_matches('/'), key)
    };
    if name.len() > 512
        || name.starts_with("aws/")
        || name.chars().any(|character| {
            !(character.is_ascii_alphanumeric()
                || matches!(character, '/' | '_' | '+' | '=' | '.' | '@' | '-'))
        })
    {
        return Err(invalid_target());
    }
    Ok(name)
}

fn validate_prefix(value: Option<&str>) -> Result<&str, ProviderPushError> {
    let value = value.unwrap_or_default().trim().trim_matches('/');
    if value.len() > 400
        || value.starts_with("aws/")
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric()
                || matches!(character, '/' | '_' | '+' | '=' | '.' | '@' | '-'))
        })
    {
        return Err(invalid_target());
    }
    Ok(value)
}

fn validate_optional_profile(value: Option<&str>) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.len() > 128 || value.chars().any(char::is_control)) {
        return Err(invalid_target());
    }
    Ok(value)
}

fn validate_optional_region(value: Option<&str>) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| {
        value.len() > 64
            || value.starts_with('-')
            || value.ends_with('-')
            || value
                .bytes()
                .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    }) {
        return Err(invalid_target());
    }
    Ok(value)
}

fn validate_optional_kms_key(value: Option<&str>) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.len() > 2048 || value.chars().any(char::is_control)) {
        return Err(invalid_target());
    }
    Ok(value)
}

fn runtime() -> Result<&'static tokio::runtime::Runtime, ProviderPushError> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("env-manager-aws")
        .build()
        .map_err(|_| ProviderPushError {
            code: "AWS_RUNTIME_UNAVAILABLE",
            message: "AWS Provider 실행 환경을 준비하지 못했습니다.",
        })?;
    let _ = RUNTIME.set(runtime);
    RUNTIME.get().ok_or(ProviderPushError {
        code: "AWS_RUNTIME_UNAVAILABLE",
        message: "AWS Provider 실행 환경을 준비하지 못했습니다.",
    })
}

fn aws_region_missing() -> ProviderPushError {
    ProviderPushError {
        code: "AWS_REGION_MISSING",
        message: "AWS Region을 입력하거나 로컬 AWS 설정에 기본 Region을 지정해주세요.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_bounded_aws_resource_names() {
        assert_eq!(
            remote_name("team/staging", "API_KEY").unwrap(),
            "team/staging/API_KEY"
        );
        assert!(remote_name("aws/reserved", "API_KEY").is_err());
        assert!(validate_optional_region(Some("ap-northeast-2")).is_ok());
        assert!(validate_optional_region(Some("AP Northeast 2")).is_err());
    }

    #[test]
    fn equality_check_is_exact_without_creating_a_digest() {
        assert!(constant_time_equal(b"fake_secret", b"fake_secret"));
        assert!(!constant_time_equal(b"fake_secret", b"fake_secreu"));
        assert!(!constant_time_equal(b"fake_secret", b"fake_secret_longer"));
    }
}

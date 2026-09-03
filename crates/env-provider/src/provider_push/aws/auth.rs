use super::*;

pub fn inspect_access(
    profile: Option<&str>,
    region: Option<&str>,
) -> Result<AwsAccessContext, ProviderPushError> {
    runtime()?.block_on(inspect_access_async(profile, region))
}

pub(super) async fn inspect_access_async(
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

pub(super) async fn load_and_verify(
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

pub(super) async fn list_kms_aliases(config: &SdkConfig) -> (Vec<String>, bool) {
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

pub(super) fn runtime() -> Result<&'static tokio::runtime::Runtime, ProviderPushError> {
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

pub(super) fn aws_region_missing() -> ProviderPushError {
    ProviderPushError {
        code: "AWS_REGION_MISSING",
        message: "AWS Region을 입력하거나 로컬 AWS 설정에 기본 Region을 지정해주세요.",
    }
}

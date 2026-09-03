use super::*;

pub(in crate::provider_push) fn prepare(
    provider: OfficialProviderId,
    request: &ProviderPushRequest,
) -> Result<PreparedAwsProvider, ProviderPushError> {
    if request
        .selections
        .iter()
        .any(|selection| selection.kind != ProviderEntryKind::Secret)
    {
        return Err(invalid_request(
            "AWS Secrets Manager와 SSM에는 Secret 유형만 전송할 수 있습니다.",
        ));
    }
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

pub(in crate::provider_push) fn push(
    values: Vec<ProviderValue>,
    prepared: PreparedAwsProvider,
) -> Result<ProviderPushResult, ProviderPushError> {
    runtime()?.block_on(push_async(values, prepared))
}

pub(super) async fn push_async(
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

pub(super) async fn push_secret(
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

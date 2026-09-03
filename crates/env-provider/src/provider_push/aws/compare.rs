use super::*;

pub(in crate::provider_push) fn compare(
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

pub(super) async fn compare_async(
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

pub(super) fn comparison_item(
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

pub(super) fn comparison_unset(key: &str, remote_name: String) -> ProviderComparisonItem {
    ProviderComparisonItem {
        key: key.to_owned(),
        remote_name,
        state: ProviderComparisonState::Unset,
        result_code: None,
    }
}

pub(super) fn comparison_error(
    key: &str,
    remote_name: String,
    result_code: &str,
) -> ProviderComparisonItem {
    ProviderComparisonItem {
        key: key.to_owned(),
        remote_name,
        state: ProviderComparisonState::Error,
        result_code: Some(result_code.to_owned()),
    }
}

pub(super) fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

use std::sync::OnceLock;

use aws_config::{BehaviorVersion, Region, SdkConfig, retry::RetryConfig};
use aws_sdk_ssm::types::ParameterType;
use env_core::ProviderValue;
use serde::Serialize;
use zeroize::Zeroizing;

use super::error::{ProviderPushError, invalid_request, invalid_target};
use super::model::{
    AWS_SECRETS_MANAGER_ID, AWS_SSM_PARAMETER_STORE_ID, OfficialProviderId, ProviderCompareRequest,
    ProviderCompareResult, ProviderComparisonItem, ProviderComparisonState, ProviderEntryKind,
    ProviderPushRequest, ProviderPushResult,
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

mod auth;
mod compare;
mod push;
mod validation;

pub use auth::inspect_access;
use auth::{aws_region_missing, load_and_verify, runtime};
pub(super) use compare::compare;
#[cfg(test)]
use compare::constant_time_equal;
pub(super) use push::{prepare, push};
use validation::{
    remote_name, validate_optional_kms_key, validate_optional_profile, validate_optional_region,
    validate_prefix,
};

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

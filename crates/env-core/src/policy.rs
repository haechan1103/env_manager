use serde::{Deserialize, Serialize};

use crate::CodexAccess;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationSuggestion {
    pub access: CodexAccess,
    pub reason: String,
}

const PROTECTED_INDICATORS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PRIVATE_KEY",
    "CLIENT_SECRET",
    "API_KEY",
    "ACCESS_KEY",
    "SESSION",
    "COOKIE",
    "CREDENTIAL",
    "DATABASE_URL",
    "DSN",
    "SIGNING_KEY",
    "ENCRYPTION_KEY",
];

const SAFE_EXACT: &[&str] = &[
    "PORT",
    "HOST",
    "HOSTNAME",
    "NODE_ENV",
    "APP_ENV",
    "LOG_LEVEL",
    "DEBUG",
    "TZ",
    "LOCALE",
];

const SAFE_PREFIXES: &[&str] = &[
    "PUBLIC_",
    "NEXT_PUBLIC_",
    "VITE_",
    "EXPO_PUBLIC_",
    "REACT_APP_",
];

const CLIENT_EXPOSED_PREFIXES: &[&str] = &["VITE_", "NEXT_PUBLIC_", "EXPO_PUBLIC_"];
const SECRET_NAME_INDICATORS: &[&str] = &["SECRET", "TOKEN", "PASSWORD", "API_KEY", "PRIVATE_KEY"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientExposureWarning {
    pub public_prefix: String,
    pub secret_indicator: String,
}

/// Detects a likely secret placed in a framework-defined client-visible variable.
/// This intentionally analyzes only the variable name; it never needs the value.
pub fn detect_client_exposure(key: &str) -> Option<ClientExposureWarning> {
    let upper = key.to_ascii_uppercase();
    let public_prefix = CLIENT_EXPOSED_PREFIXES
        .iter()
        .find(|prefix| upper.starts_with(**prefix))?;
    let secret_indicator = SECRET_NAME_INDICATORS
        .iter()
        .find(|indicator| upper.contains(**indicator))?;
    Some(ClientExposureWarning {
        public_prefix: (*public_prefix).to_owned(),
        secret_indicator: (*secret_indicator).to_owned(),
    })
}

pub fn suggest_access(key: &str) -> ClassificationSuggestion {
    let upper = key.to_ascii_uppercase();
    if let Some(indicator) = PROTECTED_INDICATORS
        .iter()
        .find(|indicator| upper.contains(**indicator))
    {
        return ClassificationSuggestion {
            access: CodexAccess::Protected,
            reason: format!("민감한 이름 패턴 `{indicator}`을 포함합니다."),
        };
    }

    if SAFE_EXACT.contains(&upper.as_str())
        || SAFE_PREFIXES.iter().any(|prefix| upper.starts_with(prefix))
    {
        return ClassificationSuggestion {
            access: CodexAccess::ReadWrite,
            reason: "일반 설정으로 보이는 이름입니다.".to_owned(),
        };
    }

    ClassificationSuggestion {
        access: CodexAccess::Unclassified,
        reason: "이름만으로 안전하게 판단할 수 없습니다.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_indicator_wins_over_public_prefix() {
        let suggestion = suggest_access("NEXT_PUBLIC_API_KEY");
        assert_eq!(suggestion.access, CodexAccess::Protected);
    }

    #[test]
    fn unknown_name_stays_unclassified() {
        let suggestion = suggest_access("CUSTOM_MODE");
        assert_eq!(suggestion.access, CodexAccess::Unclassified);
    }

    #[test]
    fn client_exposure_requires_both_public_prefix_and_secret_indicator() {
        let warning = detect_client_exposure("NEXT_PUBLIC_DATABASE_PASSWORD")
            .expect("client exposure warning");
        assert_eq!(warning.public_prefix, "NEXT_PUBLIC_");
        assert_eq!(warning.secret_indicator, "PASSWORD");
        assert!(detect_client_exposure("NEXT_PUBLIC_API_BASE_URL").is_none());
        assert!(detect_client_exposure("DATABASE_PASSWORD").is_none());
    }
}

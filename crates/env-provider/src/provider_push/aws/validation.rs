use super::*;

pub(super) fn remote_name(prefix: &str, key: &str) -> Result<String, ProviderPushError> {
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

pub(super) fn validate_prefix(value: Option<&str>) -> Result<&str, ProviderPushError> {
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

pub(super) fn validate_optional_profile(
    value: Option<&str>,
) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.len() > 128 || value.chars().any(char::is_control)) {
        return Err(invalid_target());
    }
    Ok(value)
}

pub(super) fn validate_optional_region(
    value: Option<&str>,
) -> Result<Option<&str>, ProviderPushError> {
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

pub(super) fn validate_optional_kms_key(
    value: Option<&str>,
) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.len() > 2048 || value.chars().any(char::is_control)) {
        return Err(invalid_target());
    }
    Ok(value)
}

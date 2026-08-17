use std::path::{Path, PathBuf};

use super::error::{ProviderPushError, invalid_target};

pub(super) fn source_directory(
    root: &Path,
    source_file: &str,
) -> Result<PathBuf, ProviderPushError> {
    let relative = Path::new(source_file);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(invalid_target());
    }
    let source = root.join(relative);
    Ok(source
        .parent()
        .filter(|path| path.starts_with(root))
        .unwrap_or(root)
        .to_owned())
}

pub(super) fn validate_repository(value: &str) -> Result<(), ProviderPushError> {
    let Some((owner, repository)) = value.split_once('/') else {
        return Err(invalid_target());
    };
    if owner.is_empty() || repository.is_empty() || repository.contains('/') {
        return Err(invalid_target());
    }
    validate_simple_target(owner)?;
    validate_simple_target(repository)
}

pub(super) fn optional_target(value: Option<&str>) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = value {
        validate_simple_target(value)?;
    }
    Ok(value)
}

pub(super) fn validate_simple_target(value: &str) -> Result<(), ProviderPushError> {
    let valid = !value.is_empty()
        && value.len() <= 100
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid { Ok(()) } else { Err(invalid_target()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_closed_provider_target_shapes() {
        assert!(validate_repository("owner/repository").is_ok());
        assert!(validate_repository("owner").is_err());
        assert!(validate_repository("owner/repo/extra").is_err());
        assert!(validate_repository("--owner/repository").is_err());
        assert!(validate_simple_target("worker-production").is_ok());
        assert!(validate_simple_target("--config").is_err());
        assert!(validate_simple_target("worker name").is_err());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialErrorCode {
    InvalidInput,
    AccountNotFound,
    ProjectNotAllowed,
    SecretMissing,
    SecretStoreUnavailable,
    SecretStoreFailed,
    MetadataFailed,
}

impl CredentialErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "CREDENTIAL_INVALID_INPUT",
            Self::AccountNotFound => "CREDENTIAL_NOT_FOUND",
            Self::ProjectNotAllowed => "CREDENTIAL_PROJECT_NOT_ALLOWED",
            Self::SecretMissing => "CREDENTIAL_SECRET_MISSING",
            Self::SecretStoreUnavailable => "CREDENTIAL_STORE_UNAVAILABLE",
            Self::SecretStoreFailed => "CREDENTIAL_STORE_FAILED",
            Self::MetadataFailed => "CREDENTIAL_METADATA_FAILED",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CredentialError {
    code: CredentialErrorCode,
    message: &'static str,
}

impl CredentialError {
    pub const fn new(code: CredentialErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }

    pub const fn code(&self) -> CredentialErrorCode {
        self.code
    }

    pub(crate) const fn invalid() -> Self {
        Self::new(
            CredentialErrorCode::InvalidInput,
            "계정 정보가 올바르지 않습니다.",
        )
    }

    pub(crate) const fn not_found() -> Self {
        Self::new(
            CredentialErrorCode::AccountNotFound,
            "저장된 계정을 찾지 못했습니다.",
        )
    }

    pub(crate) const fn project_not_allowed() -> Self {
        Self::new(
            CredentialErrorCode::ProjectNotAllowed,
            "이 프로젝트에는 해당 계정 사용이 허용되지 않았습니다.",
        )
    }

    pub(crate) const fn secret_missing() -> Self {
        Self::new(
            CredentialErrorCode::SecretMissing,
            "운영체제 보안 저장소에서 계정 값을 찾지 못했습니다.",
        )
    }

    pub(crate) const fn store_unavailable() -> Self {
        Self::new(
            CredentialErrorCode::SecretStoreUnavailable,
            "운영체제 보안 저장소를 사용할 수 없습니다.",
        )
    }

    pub(crate) const fn store_failed() -> Self {
        Self::new(
            CredentialErrorCode::SecretStoreFailed,
            "운영체제 보안 저장소 작업에 실패했습니다.",
        )
    }

    pub(crate) const fn metadata_failed() -> Self {
        Self::new(
            CredentialErrorCode::MetadataFailed,
            "로컬 계정 설정을 저장하지 못했습니다.",
        )
    }
}

pub type CredentialResult<T> = Result<T, CredentialError>;

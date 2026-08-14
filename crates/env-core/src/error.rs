use std::io;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

pub type EnvResult<T> = Result<T, EnvError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvErrorCode {
    FileChangedExternally,
    ParseAmbiguousDuplicateKey,
    ParseUnsupported,
    PathOutsideRegisteredProject,
    UnsupportedSymlink,
    UnsupportedEncoding,
    FileTooLarge,
    LinkValueConflict,
    LinkMemberMissing,
    CodexAccessBlocked,
    ProtectionDowngradeRequiresConfirmation,
    PlanExpired,
    MultiFileCommitFailed,
    UnregisteredProject,
    InvalidRequest,
    Io,
    Serialization,
    PackageDecryptFailed,
    PackageInvalid,
    PackageConflict,
}

impl EnvErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileChangedExternally => "FILE_CHANGED_EXTERNALLY",
            Self::ParseAmbiguousDuplicateKey => "PARSE_AMBIGUOUS_DUPLICATE_KEY",
            Self::ParseUnsupported => "PARSE_UNSUPPORTED",
            Self::PathOutsideRegisteredProject => "PATH_OUTSIDE_REGISTERED_PROJECT",
            Self::UnsupportedSymlink => "UNSUPPORTED_SYMLINK",
            Self::UnsupportedEncoding => "UNSUPPORTED_ENCODING",
            Self::FileTooLarge => "FILE_TOO_LARGE",
            Self::LinkValueConflict => "LINK_VALUE_CONFLICT",
            Self::LinkMemberMissing => "LINK_MEMBER_MISSING",
            Self::CodexAccessBlocked => "CODEX_ACCESS_BLOCKED",
            Self::ProtectionDowngradeRequiresConfirmation => {
                "PROTECTION_DOWNGRADE_REQUIRES_CONFIRMATION"
            }
            Self::PlanExpired => "PLAN_EXPIRED",
            Self::MultiFileCommitFailed => "MULTI_FILE_COMMIT_FAILED",
            Self::UnregisteredProject => "UNREGISTERED_PROJECT",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::Io => "IO_ERROR",
            Self::Serialization => "SERIALIZATION_ERROR",
            Self::PackageDecryptFailed => "PACKAGE_DECRYPT_FAILED",
            Self::PackageInvalid => "PACKAGE_INVALID",
            Self::PackageConflict => "PACKAGE_CONFLICT",
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct EnvError {
    code: EnvErrorCode,
    message: String,
}

impl EnvError {
    pub fn new(code: EnvErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> EnvErrorCode {
        self.code
    }

    pub fn io(path: &Path, _source: io::Error) -> Self {
        Self::new(
            EnvErrorCode::Io,
            format!("파일 작업에 실패했습니다: {}", display_path(path)),
        )
    }

    pub fn serialization(_source: serde_json::Error) -> Self {
        Self::new(
            EnvErrorCode::Serialization,
            "설정 데이터를 처리하지 못했습니다.",
        )
    }

    pub fn path_outside(path: &Path) -> Self {
        Self::new(
            EnvErrorCode::PathOutsideRegisteredProject,
            format!("등록된 프로젝트 밖의 경로입니다: {}", display_path(path)),
        )
    }

    pub fn unsupported_encoding(path: &Path) -> Self {
        Self::new(
            EnvErrorCode::UnsupportedEncoding,
            format!(
                "UTF-8 env 파일만 편집할 수 있습니다: {}",
                display_path(path)
            ),
        )
    }

    pub fn file_too_large(path: &Path) -> Self {
        Self::new(
            EnvErrorCode::FileTooLarge,
            format!(
                "env 파일이 허용된 크기를 초과했습니다: {}",
                display_path(path)
            ),
        )
    }

    pub fn duplicate_key(key: &str, path: &Path) -> Self {
        Self::new(
            EnvErrorCode::ParseAmbiguousDuplicateKey,
            format!(
                "같은 파일에 {key} 변수가 여러 번 있습니다: {}",
                display_path(path)
            ),
        )
    }

    pub fn link_conflict(key: &str) -> Self {
        Self::new(
            EnvErrorCode::LinkValueConflict,
            format!("{key} 연결 대상에 서로 다른 값이 있습니다."),
        )
    }

    pub fn link_member_missing(key: &str, path: &Path) -> Self {
        Self::new(
            EnvErrorCode::LinkMemberMissing,
            format!("{key} 변수를 찾지 못했습니다: {}", display_path(path)),
        )
    }

    pub fn changed_externally(path: &Path) -> Self {
        Self::new(
            EnvErrorCode::FileChangedExternally,
            format!("파일이 외부에서 변경되었습니다: {}", display_path(path)),
        )
    }

    pub fn access_blocked(key: &str) -> Self {
        Self::new(
            EnvErrorCode::CodexAccessBlocked,
            format!("{key} 값은 Codex 접근이 차단되어 있습니다."),
        )
    }

    pub fn confirmation_required(key: impl AsRef<str>) -> Self {
        Self::new(
            EnvErrorCode::ProtectionDowngradeRequiresConfirmation,
            format!(
                "{} 변수를 Codex 열람 가능으로 바꾸려면 명시적 확인이 필요합니다.",
                key.as_ref()
            ),
        )
    }

    pub fn unregistered_project(project_id: &str) -> Self {
        Self::new(
            EnvErrorCode::UnregisteredProject,
            format!("등록되지 않은 프로젝트입니다: {project_id}"),
        )
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(EnvErrorCode::InvalidRequest, message)
    }

    pub fn transaction(message: impl Into<String>) -> Self {
        Self::new(EnvErrorCode::MultiFileCommitFailed, message)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

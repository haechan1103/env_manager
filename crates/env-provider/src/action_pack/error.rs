use env_core::EnvError;

#[derive(Debug)]
pub struct ActionPackError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ActionPackError {
    pub(crate) const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl From<EnvError> for ActionPackError {
    fn from(error: EnvError) -> Self {
        let (code, message) = match error.code().as_str() {
            "PATH_OUTSIDE_REGISTERED_PROJECT" => (
                "PATH_OUTSIDE_REGISTERED_PROJECT",
                "등록된 프로젝트 밖의 값은 Action에 사용할 수 없습니다.",
            ),
            "PARSE_AMBIGUOUS_DUPLICATE_KEY" => (
                "PARSE_AMBIGUOUS_DUPLICATE_KEY",
                "중복된 환경변수는 Action에 안전하게 사용할 수 없습니다.",
            ),
            _ => (
                "ACTION_VALUE_SELECTION_FAILED",
                "Action에 사용할 환경변수를 준비하지 못했습니다.",
            ),
        };
        Self { code, message }
    }
}

pub(crate) fn invalid_pack() -> ActionPackError {
    ActionPackError::new(
        "ACTION_PACK_INVALID",
        "Action Pack 형식이나 보안 규칙이 올바르지 않습니다.",
    )
}

pub(crate) fn invalid_request() -> ActionPackError {
    ActionPackError::new(
        "ACTION_REQUEST_INVALID",
        "Action Pack과 환경변수 연결을 다시 확인해주세요.",
    )
}

pub(crate) fn storage_failed() -> ActionPackError {
    ActionPackError::new(
        "ACTION_PACK_STORAGE_FAILED",
        "Action Pack을 로컬 저장소에 반영하지 못했습니다.",
    )
}

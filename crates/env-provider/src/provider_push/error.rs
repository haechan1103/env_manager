use env_core::EnvError;

#[derive(Debug)]
pub struct ProviderPushError {
    pub code: &'static str,
    pub message: &'static str,
}

pub(super) fn invalid_target() -> ProviderPushError {
    invalid_request("배포 대상 형식이 올바르지 않습니다.")
}

pub(super) fn invalid_request(message: &'static str) -> ProviderPushError {
    ProviderPushError {
        code: "INVALID_REQUEST",
        message,
    }
}

impl From<EnvError> for ProviderPushError {
    fn from(error: EnvError) -> Self {
        let code = match error.code().as_str() {
            "INVALID_REQUEST" => "INVALID_REQUEST",
            "PATH_OUTSIDE_REGISTERED_PROJECT" => "PATH_OUTSIDE_REGISTERED_PROJECT",
            "PARSE_AMBIGUOUS_DUPLICATE_KEY" => "PARSE_AMBIGUOUS_DUPLICATE_KEY",
            _ => "PROVIDER_SELECTION_FAILED",
        };
        let message = match code {
            "INVALID_REQUEST" => "전송할 파일과 변수를 다시 확인해주세요.",
            "PATH_OUTSIDE_REGISTERED_PROJECT" => "등록된 프로젝트 밖의 파일은 전송할 수 없습니다.",
            "PARSE_AMBIGUOUS_DUPLICATE_KEY" => "중복된 변수는 안전하게 전송할 수 없습니다.",
            _ => "전송할 환경변수를 준비하지 못했습니다.",
        };
        Self { code, message }
    }
}

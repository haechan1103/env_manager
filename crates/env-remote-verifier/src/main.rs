use std::path::Path;
use std::process::ExitCode;

use env_remote_verifier::{VerifierError, compare_encrypted_request, load_config, read_bounded};
use serde::Serialize;

const DEFAULT_CONFIG_PATH: &str = "/etc/env-manager/verifier.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    protocol_version: u32,
    result_code: &'static str,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let response = ErrorResponse {
                protocol_version: 1,
                result_code: error.code(),
            };
            if let Ok(serialized) = serde_json::to_string(&response) {
                println!("{serialized}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), VerifierError> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("compare"))
        || arguments.next().is_some()
    {
        return Err(VerifierError::Stable {
            code: "REMOTE_COMMAND_INVALID",
        });
    }
    let config = load_config(Path::new(DEFAULT_CONFIG_PATH))?;
    let encrypted = read_bounded(std::io::stdin().lock())?;
    let response = compare_encrypted_request(&config, &encrypted)?;
    let serialized = serde_json::to_string(&response).map_err(|_| VerifierError::Stable {
        code: "REMOTE_RESPONSE_SERIALIZATION_FAILED",
    })?;
    println!("{serialized}");
    Ok(())
}

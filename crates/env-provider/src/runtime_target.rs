use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use env_core::{ProviderValue, is_env_candidate};
use env_remote_verifier::{VerifierResponse, VerifierState, encrypt_request, validate_recipient};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;
use zeroize::Zeroizing;

use crate::provider_push::cli;
use crate::provider_push::{
    ProviderCompareResult, ProviderComparisonItem, ProviderComparisonState, ProviderPushError,
};

pub const REMOTE_RUNTIME_PROVIDER_ID: &str = "remote-runtime";
pub const REMOTE_TARGETS_FILE_NAME: &str = ".env-manager.remotes.json";
const REMOTE_COMMAND: &str = "env-manager-remote-verifier";
const REMOTE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeTargetFile {
    version: u32,
    #[serde(default)]
    targets: Vec<RuntimeTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeTarget {
    pub id: String,
    pub display_name: String,
    pub source_file: String,
    pub remote_target_id: String,
    pub recipient: String,
    pub transport: RuntimeTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RuntimeTransport {
    Ssh {
        destination: String,
    },
    Ecs {
        cluster: String,
        task: String,
        container: Option<String>,
        profile: Option<String>,
        region: Option<String>,
    },
}

impl RuntimeTarget {
    pub fn transport_label(&self) -> &'static str {
        match self.transport {
            RuntimeTransport::Ssh { .. } => "SSH",
            RuntimeTransport::Ecs { .. } => "ECS",
        }
    }
}

pub fn list(root: &Path) -> Result<Vec<RuntimeTarget>, ProviderPushError> {
    load(root).map(|config| config.targets)
}

pub fn save(root: &Path, target: RuntimeTarget) -> Result<Vec<RuntimeTarget>, ProviderPushError> {
    validate_target(&target)?;
    let mut config = load(root)?;
    if let Some(existing) = config.targets.iter_mut().find(|item| item.id == target.id) {
        *existing = target;
    } else {
        config.targets.push(target);
    }
    config.targets.sort_by(|left, right| left.id.cmp(&right.id));
    persist(root, &config)?;
    Ok(config.targets)
}

pub fn remove(root: &Path, id: &str) -> Result<Vec<RuntimeTarget>, ProviderPushError> {
    validate_identifier(id)?;
    let mut config = load(root)?;
    config.targets.retain(|target| target.id != id);
    persist(root, &config)?;
    Ok(config.targets)
}

pub fn compare(
    root: &Path,
    target_id: &str,
    source_file: &str,
    values: Vec<ProviderValue>,
) -> Result<ProviderCompareResult, ProviderPushError> {
    let target = load(root)?
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or(ProviderPushError {
            code: "REMOTE_TARGET_UNKNOWN",
            message: "등록된 원격 Runtime 대상을 찾지 못했습니다.",
        })?;
    if target.source_file != source_file {
        return Err(ProviderPushError {
            code: "REMOTE_SOURCE_MISMATCH",
            message: "이 Runtime 대상에 연결된 로컬 env 파일이 아닙니다.",
        });
    }
    match &target.transport {
        RuntimeTransport::Ssh { destination } => compare_ssh(root, &target, destination, values),
        RuntimeTransport::Ecs { .. } => Err(ProviderPushError {
            code: "REMOTE_ECS_TRANSPORT_UNAVAILABLE",
            message: "ECS Runtime 전송은 아직 활성화되지 않았습니다.",
        }),
    }
}

fn compare_ssh(
    root: &Path,
    target: &RuntimeTarget,
    destination: &str,
    values: Vec<ProviderValue>,
) -> Result<ProviderCompareResult, ProviderPushError> {
    let ssh = cli::find_cli("ssh", root).ok_or(ProviderPushError {
        code: "SSH_CLI_UNAVAILABLE",
        message: "SSH 실행 파일을 찾지 못했습니다.",
    })?;
    let encrypted = Zeroizing::new(
        encrypt_request(
            &target.recipient,
            &target.remote_target_id,
            values.iter().map(|value| (value.key(), value.value())),
        )
        .map_err(remote_protocol_error)?,
    );
    let args = [
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=5",
        "-o",
        "ServerAliveCountMax=1",
        "-o",
        "LogLevel=ERROR",
        destination,
        REMOTE_COMMAND,
        "compare",
    ]
    .map(OsString::from);
    let response = run_ssh(&ssh, root, &args, &encrypted)?;
    validate_response(target, &values, response)
}

fn run_ssh(
    executable: &Path,
    root: &Path,
    args: &[OsString],
    stdin: &[u8],
) -> Result<VerifierResponse, ProviderPushError> {
    let mut child = cli::provider_command(executable, args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| remote_transport_failed())?;
    child
        .stdin
        .take()
        .ok_or_else(remote_transport_failed)?
        .write_all(stdin)
        .map_err(|_| remote_transport_failed())?;
    let status = child
        .wait_timeout(REMOTE_TIMEOUT)
        .map_err(|_| remote_transport_failed())?;
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ProviderPushError {
            code: "REMOTE_TRANSPORT_TIMEOUT",
            message: "원격 Runtime 확인 시간이 초과됐습니다.",
        });
    }
    let output = child
        .wait_with_output()
        .map_err(|_| remote_transport_failed())?;
    if !output.status.success() || output.stdout.len() > MAX_RESPONSE_BYTES {
        return Err(remote_transport_failed());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| ProviderPushError {
        code: "REMOTE_RESPONSE_INVALID",
        message: "원격 Verifier 응답 형식이 올바르지 않습니다.",
    })
}

fn validate_response(
    target: &RuntimeTarget,
    values: &[ProviderValue],
    response: VerifierResponse,
) -> Result<ProviderCompareResult, ProviderPushError> {
    if response.protocol_version != 1 || response.target_id != target.remote_target_id {
        return Err(ProviderPushError {
            code: "REMOTE_RESPONSE_MISMATCH",
            message: "요청한 Runtime 대상과 원격 응답이 일치하지 않습니다.",
        });
    }
    let expected = values
        .iter()
        .map(|value| value.key())
        .collect::<BTreeSet<_>>();
    let received = response
        .comparisons
        .iter()
        .map(|item| item.key.as_str())
        .collect::<BTreeSet<_>>();
    if expected.len() != response.comparisons.len() || expected != received {
        return Err(ProviderPushError {
            code: "REMOTE_RESPONSE_MISMATCH",
            message: "원격 Verifier가 다른 변수 목록을 응답했습니다.",
        });
    }
    Ok(ProviderCompareResult {
        provider: REMOTE_RUNTIME_PROVIDER_ID.to_owned(),
        target: target.display_name.clone(),
        items: response
            .comparisons
            .into_iter()
            .map(|item| ProviderComparisonItem {
                remote_name: item.key.clone(),
                key: item.key,
                state: match item.state {
                    VerifierState::Same => ProviderComparisonState::Same,
                    VerifierState::Different => ProviderComparisonState::Different,
                    VerifierState::Unset => ProviderComparisonState::Unset,
                    VerifierState::Error => ProviderComparisonState::Error,
                },
                result_code: item.result_code,
            })
            .collect(),
    })
}

fn load(root: &Path) -> Result<RuntimeTargetFile, ProviderPushError> {
    let path = root.join(REMOTE_TARGETS_FILE_NAME);
    if !path.exists() {
        return Ok(RuntimeTargetFile {
            version: 1,
            targets: Vec::new(),
        });
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| config_unavailable())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(config_unavailable());
    }
    let config = serde_json::from_slice::<RuntimeTargetFile>(
        &fs::read(&path).map_err(|_| config_unavailable())?,
    )
    .map_err(|_| config_invalid())?;
    if config.version != 1 {
        return Err(config_invalid());
    }
    let mut ids = BTreeSet::new();
    for target in &config.targets {
        validate_target(target)?;
        if !ids.insert(target.id.as_str()) {
            return Err(config_invalid());
        }
    }
    Ok(config)
}

fn persist(root: &Path, config: &RuntimeTargetFile) -> Result<(), ProviderPushError> {
    let path = root.join(REMOTE_TARGETS_FILE_NAME);
    let mut staged = NamedTempFile::new_in(root).map_err(|_| config_unavailable())?;
    serde_json::to_writer_pretty(staged.as_file_mut(), config).map_err(|_| config_invalid())?;
    staged
        .as_file_mut()
        .write_all(b"\n")
        .map_err(|_| config_unavailable())?;
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|_| config_unavailable())?;
    staged.persist(path).map_err(|_| config_unavailable())?;
    Ok(())
}

fn validate_target(target: &RuntimeTarget) -> Result<(), ProviderPushError> {
    validate_identifier(&target.id)?;
    validate_identifier(&target.remote_target_id)?;
    validate_recipient(&target.recipient).map_err(remote_protocol_error)?;
    if target.display_name.trim().is_empty()
        || target.display_name.chars().count() > 80
        || Path::new(&target.source_file).is_absolute()
        || target.source_file.split('/').any(|segment| segment == "..")
        || !Path::new(&target.source_file)
            .file_name()
            .is_some_and(|name| is_env_candidate(&name.to_string_lossy()))
    {
        return Err(config_invalid());
    }
    match &target.transport {
        RuntimeTransport::Ssh { destination } => validate_ssh_destination(destination),
        RuntimeTransport::Ecs {
            cluster,
            task,
            container,
            profile,
            region,
        } => {
            for value in [
                Some(cluster.as_str()),
                Some(task.as_str()),
                container.as_deref(),
                profile.as_deref(),
                region.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                if value.is_empty()
                    || value.len() > 512
                    || value.chars().any(char::is_whitespace)
                    || value.starts_with('-')
                {
                    return Err(config_invalid());
                }
            }
            Ok(())
        }
    }
}

fn validate_ssh_destination(destination: &str) -> Result<(), ProviderPushError> {
    if destination.is_empty()
        || destination.len() > 255
        || destination.starts_with('-')
        || !destination.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'-' | b'_' | b'@' | b':' | b'[' | b']')
        })
    {
        return Err(config_invalid());
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), ProviderPushError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(config_invalid());
    }
    Ok(())
}

fn remote_protocol_error(error: env_remote_verifier::VerifierError) -> ProviderPushError {
    ProviderPushError {
        code: error.code(),
        message: "원격 Verifier 요청을 안전하게 준비하지 못했습니다.",
    }
}

fn remote_transport_failed() -> ProviderPushError {
    ProviderPushError {
        code: "REMOTE_TRANSPORT_FAILED",
        message: "원격 Verifier에 안전하게 연결하지 못했습니다.",
    }
}

fn config_invalid() -> ProviderPushError {
    ProviderPushError {
        code: "REMOTE_TARGET_CONFIG_INVALID",
        message: "원격 Runtime 대상 설정이 올바르지 않습니다.",
    }
}

fn config_unavailable() -> ProviderPushError {
    ProviderPushError {
        code: "REMOTE_TARGET_CONFIG_UNAVAILABLE",
        message: "원격 Runtime 대상 설정을 읽거나 저장하지 못했습니다.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret;

    #[test]
    fn shared_target_config_round_trips_without_private_identity_or_values() {
        let project = tempfile::tempdir().expect("project");
        let identity = age::x25519::Identity::generate();
        let target = RuntimeTarget {
            id: "mobile-ok-dev".to_owned(),
            display_name: "mobile-ok · dev".to_owned(),
            source_file: ".env.local".to_owned(),
            remote_target_id: "mobile-ok-dev".to_owned(),
            recipient: identity.to_public().to_string(),
            transport: RuntimeTransport::Ssh {
                destination: "deploy@example.test".to_owned(),
            },
        };

        let stored = save(project.path(), target.clone()).expect("save target");
        assert_eq!(stored, vec![target]);
        let serialized =
            fs::read_to_string(project.path().join(REMOTE_TARGETS_FILE_NAME)).expect("read config");
        assert!(serialized.contains("mobile-ok-dev"));
        assert!(!serialized.contains(identity.to_string().expose_secret()));
        assert!(!serialized.contains("value"));
    }

    #[test]
    fn target_rejects_command_shaped_ssh_destination() {
        let identity = age::x25519::Identity::generate();
        let target = RuntimeTarget {
            id: "dev".to_owned(),
            display_name: "Dev".to_owned(),
            source_file: "runtime.env".to_owned(),
            remote_target_id: "dev".to_owned(),
            recipient: identity.to_public().to_string(),
            transport: RuntimeTransport::Ssh {
                destination: "deploy@example.test;touch /tmp/bad".to_owned(),
            },
        };
        assert_eq!(
            validate_target(&target)
                .expect_err("reject command shape")
                .code,
            "REMOTE_TARGET_CONFIG_INVALID"
        );
    }
}

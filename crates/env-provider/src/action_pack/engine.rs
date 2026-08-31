use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::process::Stdio;
use std::time::{Duration, Instant};

use env_core::{ProjectService, ProviderValue};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use wait_timeout::ChildExt;
use zeroize::Zeroizing;

use super::error::{ActionPackError, invalid_request};
use super::model::{
    ActionDefinition, ActionExecutionRequest, ActionExecutionResult, ActionKind, ActionPackInfo,
    HttpActionMethod,
};
use super::storage::{ResolvedActionPack, pack_info, resolve};
use crate::provider_push::cli::provider_command;

pub fn prepare(
    root: &std::path::Path,
    app_data: &std::path::Path,
    request: &ActionExecutionRequest,
) -> Result<ActionPackInfo, ActionPackError> {
    let resolved = resolve(&request.pack_id, root, app_data)?;
    validate_bindings(&resolved, &request.bindings)?;
    Ok(pack_info(&resolved.manifest, Some(&resolved)))
}

pub fn execute(
    service: &ProjectService,
    app_data: &std::path::Path,
    request: ActionExecutionRequest,
) -> Result<ActionExecutionResult, ActionPackError> {
    let resolved = resolve(&request.pack_id, service.root(), app_data)?;
    validate_bindings(&resolved, &request.bindings)?;
    let keys = request
        .bindings
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let values = service.provider_values(&request.file, &keys)?;
    let values = values
        .iter()
        .map(|value| (value.key(), value))
        .collect::<BTreeMap<_, _>>();

    match &resolved.manifest.action {
        ActionDefinition::Cli { .. } => execute_cli(service, &resolved, &request, &values),
        ActionDefinition::Http { .. } => execute_http(&resolved, &request, &values),
    }
}

fn validate_bindings(
    resolved: &ResolvedActionPack,
    bindings: &BTreeMap<String, String>,
) -> Result<(), ActionPackError> {
    let expected = match &resolved.manifest.action {
        ActionDefinition::Cli { secret_binding, .. } => BTreeSet::from([secret_binding.as_str()]),
        ActionDefinition::Http {
            secret_bindings, ..
        } => secret_bindings.keys().map(String::as_str).collect(),
    };
    let received = bindings.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != received
        || bindings.values().any(|key| {
            key.is_empty()
                || key.len() > 256
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        })
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn execute_cli(
    service: &ProjectService,
    resolved: &ResolvedActionPack,
    request: &ActionExecutionRequest,
    values: &BTreeMap<&str, &ProviderValue>,
) -> Result<ActionExecutionResult, ActionPackError> {
    let ActionDefinition::Cli {
        secret_binding,
        result_policy,
        timeout_seconds,
        ..
    } = &resolved.manifest.action
    else {
        return Err(invalid_request());
    };
    let cli = resolved.cli.as_ref().ok_or(ActionPackError::new(
        "ACTION_CLI_UNSUPPORTED",
        "Action CLI를 실행할 수 없습니다.",
    ))?;
    let variable_name = request
        .bindings
        .get(secret_binding)
        .ok_or_else(invalid_request)?;
    let value = values
        .get(variable_name.as_str())
        .ok_or_else(invalid_request)?;
    let args = cli
        .profile
        .arguments
        .iter()
        .map(|argument| {
            let rendered = argument.replace("{variableName}", variable_name);
            if rendered.contains(['{', '}']) || rendered.len() > 512 {
                return Err(invalid_request());
            }
            Ok(OsString::from(rendered))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let started = Instant::now();
    let mut command = provider_command(&cli.executable, &args);
    for variable in [
        "DEBUG",
        "NODE_DEBUG",
        "RUST_LOG",
        "SSLKEYLOGFILE",
        "WRANGLER_LOG",
    ] {
        command.env_remove(variable);
    }
    let mut child = command
        .current_dir(service.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            ActionPackError::new("ACTION_CLI_FAILED", "Action CLI를 시작하지 못했습니다.")
        })?;
    let mut stdin = child.stdin.take().ok_or(ActionPackError::new(
        "ACTION_CLI_FAILED",
        "Action CLI의 비밀 입력 채널을 열지 못했습니다.",
    ))?;
    let timeout = Duration::from_secs(*timeout_seconds);
    let (status, wrote) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(value.value().as_bytes()).is_ok());
        let status = child.wait_timeout(timeout);
        if matches!(status, Ok(None)) {
            let _ = child.kill();
            let _ = child.wait();
        }
        let wrote = writer.join().unwrap_or(false);
        (status, wrote)
    });
    let elapsed = elapsed_ms(started);
    let status = status.map_err(|_| {
        ActionPackError::new(
            "ACTION_CLI_FAILED",
            "Action CLI 실행 상태를 확인하지 못했습니다.",
        )
    })?;
    let Some(status) = status else {
        return Ok(ActionExecutionResult {
            pack_id: resolved.manifest.id.clone(),
            kind: ActionKind::Cli,
            succeeded: false,
            status_code: None,
            duration_ms: result_policy.duration.then_some(elapsed),
            exit_code: None,
            result_code: "ACTION_TIMEOUT".to_owned(),
        });
    };
    if !wrote {
        return Err(ActionPackError::new(
            "ACTION_CLI_FAILED",
            "Action CLI에 값을 전달하지 못했습니다.",
        ));
    }
    let succeeded = status.success();
    Ok(ActionExecutionResult {
        pack_id: resolved.manifest.id.clone(),
        kind: ActionKind::Cli,
        succeeded,
        status_code: None,
        duration_ms: result_policy.duration.then_some(elapsed),
        exit_code: result_policy.exit_code.then(|| status.code()).flatten(),
        result_code: if succeeded {
            "ACTION_SUCCEEDED"
        } else {
            "ACTION_CLI_EXITED"
        }
        .to_owned(),
    })
}

fn execute_http(
    resolved: &ResolvedActionPack,
    request: &ActionExecutionRequest,
    values: &BTreeMap<&str, &ProviderValue>,
) -> Result<ActionExecutionResult, ActionPackError> {
    let ActionDefinition::Http {
        method,
        url,
        secret_bindings,
        result_policy,
        timeout_seconds,
    } = &resolved.manifest.action
    else {
        return Err(invalid_request());
    };
    let mut headers = HeaderMap::new();
    for (binding_id, binding) in secret_bindings {
        let variable_name = request
            .bindings
            .get(binding_id)
            .ok_or_else(invalid_request)?;
        let value = values
            .get(variable_name.as_str())
            .ok_or_else(invalid_request)?;
        let rendered = Zeroizing::new(binding.format.replace("{value}", value.value()));
        let name = binding.name.as_deref().unwrap_or(binding_id);
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid_request())?;
        let mut header_value = HeaderValue::from_str(rendered.as_str()).map_err(|_| {
            ActionPackError::new(
                "ACTION_VALUE_UNREPRESENTABLE",
                "선택한 값을 HTTP 헤더로 안전하게 표현할 수 없습니다.",
            )
        })?;
        header_value.set_sensitive(true);
        headers.insert(name, header_value);
    }

    ensure_http_crypto_provider()?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .referer(false)
        .timeout(Duration::from_secs(*timeout_seconds))
        .build()
        .map_err(|_| {
            ActionPackError::new("ACTION_HTTP_FAILED", "HTTP Action을 준비하지 못했습니다.")
        })?;
    let started = Instant::now();
    let response = client
        .request(http_method(*method), url)
        .headers(headers)
        .send()
        .map_err(|_| {
            ActionPackError::new(
                "ACTION_HTTP_FAILED",
                "HTTP Action 요청을 완료하지 못했습니다.",
            )
        })?;
    let elapsed = elapsed_ms(started);
    let status = response.status().as_u16();
    let succeeded = if result_policy.success_status_codes.is_empty() {
        response.status().is_success()
    } else {
        result_policy.success_status_codes.contains(&status)
    };
    drop(response);

    Ok(ActionExecutionResult {
        pack_id: resolved.manifest.id.clone(),
        kind: ActionKind::Http,
        succeeded,
        status_code: result_policy.status.then_some(status),
        duration_ms: result_policy.duration.then_some(elapsed),
        exit_code: None,
        result_code: if succeeded {
            "ACTION_SUCCEEDED"
        } else {
            "ACTION_HTTP_STATUS_REJECTED"
        }
        .to_owned(),
    })
}

fn ensure_http_crypto_provider() -> Result<(), ActionPackError> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        return Err(ActionPackError::new(
            "ACTION_HTTP_FAILED",
            "HTTP Action의 암호화 구성을 준비하지 못했습니다.",
        ));
    }
    Ok(())
}

fn http_method(method: HttpActionMethod) -> reqwest::Method {
    match method {
        HttpActionMethod::Get => reqwest::Method::GET,
        HttpActionMethod::Head => reqwest::Method::HEAD,
        HttpActionMethod::Post => reqwest::Method::POST,
        HttpActionMethod::Put => reqwest::Method::PUT,
        HttpActionMethod::Patch => reqwest::Method::PATCH,
        HttpActionMethod::Delete => reqwest::Method::DELETE,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::action_pack::{
        ActionPackManifest, CliActionProfile, CliResultPolicy, CliSecretTransport,
        HttpActionMethod, HttpResultPolicy, HttpSecretBinding, HttpSecretSource, install,
    };

    #[test]
    fn http_action_returns_only_allowlisted_metadata_even_when_the_body_echoes_the_secret() {
        let canary = "fake_ACTION_PACK_SECRET_92";
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).expect("read request");
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {canary}").to_ascii_lowercase())
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                canary.len(),
                canary
            )
            .expect("response");
        });

        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join(".env.local"),
            format!("SERVICE_API_KEY={canary}\n"),
        )
        .expect("fixture");
        let service = ProjectService::open(project.path()).expect("service");
        service.initialize().expect("initialize");
        let app_data = tempfile::tempdir().expect("app data");
        let source = tempfile::tempdir().expect("source");
        let manifest = ActionPackManifest {
            schema_version: 1,
            id: "local.example.api-check".to_owned(),
            display_name: "API check".to_owned(),
            description: "Synthetic API check".to_owned(),
            pack_version: "1.0.0".to_owned(),
            action_protocol_version: "0.1.0".to_owned(),
            action: ActionDefinition::Http {
                method: HttpActionMethod::Get,
                url: format!("http://{address}/health"),
                secret_bindings: BTreeMap::from([(
                    "Authorization".to_owned(),
                    HttpSecretBinding {
                        source: HttpSecretSource::Header,
                        name: None,
                        format: "Bearer {value}".to_owned(),
                    },
                )]),
                result_policy: HttpResultPolicy {
                    status: true,
                    duration: true,
                    body: false,
                    success_status_codes: vec![200],
                },
                timeout_seconds: 5,
            },
        };
        fs::write(
            source.path().join("action.json"),
            serde_json::to_vec(&manifest).expect("manifest"),
        )
        .expect("write manifest");
        install(source.path(), app_data.path(), false).expect("install");

        let result = execute(
            &service,
            app_data.path(),
            ActionExecutionRequest {
                pack_id: manifest.id,
                file: ".env.local".to_owned(),
                bindings: BTreeMap::from([(
                    "Authorization".to_owned(),
                    "SERVICE_API_KEY".to_owned(),
                )]),
            },
        )
        .expect("execute");
        server.join().expect("server");

        assert!(result.succeeded);
        assert_eq!(result.status_code, Some(200));
        assert!(
            !serde_json::to_string(&result)
                .expect("result")
                .contains(canary)
        );
    }

    #[cfg(unix)]
    #[test]
    fn cli_action_uses_name_only_arguments_and_discards_secret_bearing_output() {
        use std::os::unix::fs::PermissionsExt;

        let canary = "fake_ACTION_CLI_SECRET_81";
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join(".env.local"),
            format!("SERVICE_API_KEY={canary}\n"),
        )
        .expect("fixture");
        let service = ProjectService::open(project.path()).expect("service");
        service.initialize().expect("initialize");
        let app_data = tempfile::tempdir().expect("app data");
        let source = tempfile::tempdir().expect("source");
        let runner = tempfile::tempdir().expect("runner");
        let executable = runner.path().join("fake-action");
        let stdin_capture = runner.path().join("stdin.txt");
        let name_capture = runner.path().join("name.txt");
        fs::write(
            &executable,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '2.1.0\\n'; exit 0; fi\nstdin_file=$1\nname_file=$2\nvariable_name=$3\nprintf '%s' \"$variable_name\" > \"$name_file\"\ncat > \"$stdin_file\"\nprintf '%s' \"$(cat \"$stdin_file\")\"\nprintf '%s' \"$(cat \"$stdin_file\")\" >&2\n",
        )
        .expect("runner source");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("permissions");
        let manifest = ActionPackManifest {
            schema_version: 1,
            id: "local.example.cli-check".to_owned(),
            display_name: "CLI check".to_owned(),
            description: "Synthetic CLI action".to_owned(),
            pack_version: "1.0.0".to_owned(),
            action_protocol_version: "0.1.0".to_owned(),
            action: ActionDefinition::Cli {
                executable_candidates: vec![executable.to_string_lossy().into_owned()],
                version_args: vec!["--version".to_owned()],
                profiles: vec![CliActionProfile {
                    id: "fake-v2".to_owned(),
                    version_requirement: ">=2,<3".to_owned(),
                    arguments: vec![
                        stdin_capture.to_string_lossy().into_owned(),
                        name_capture.to_string_lossy().into_owned(),
                        "{variableName}".to_owned(),
                    ],
                }],
                secret_binding: "value".to_owned(),
                secret_transport: CliSecretTransport::Stdin,
                result_policy: CliResultPolicy {
                    success: true,
                    exit_code: true,
                    duration: true,
                },
                timeout_seconds: 5,
            },
        };
        fs::write(
            source.path().join("action.json"),
            serde_json::to_vec(&manifest).expect("manifest"),
        )
        .expect("write manifest");
        install(source.path(), app_data.path(), false).expect("install");

        let result = execute(
            &service,
            app_data.path(),
            ActionExecutionRequest {
                pack_id: manifest.id,
                file: ".env.local".to_owned(),
                bindings: BTreeMap::from([("value".to_owned(), "SERVICE_API_KEY".to_owned())]),
            },
        )
        .expect("execute");

        assert!(result.succeeded);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(fs::read_to_string(stdin_capture).expect("stdin"), canary);
        assert_eq!(
            fs::read_to_string(name_capture).expect("name"),
            "SERVICE_API_KEY"
        );
        assert!(
            !serde_json::to_string(&result)
                .expect("result")
                .contains(canary)
        );
    }
}

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use directories::BaseDirs;
use env_core::{EnvError, ProjectService, ProviderValue};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentProviderId {
    GithubActions,
    CloudflareWorkers,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GitHubEntryKind {
    Secret,
    Variable,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelection {
    pub key: String,
    pub kind: GitHubEntryKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPushRequest {
    pub provider: DeploymentProviderId,
    pub file: String,
    pub selections: Vec<ProviderSelection>,
    pub repository: Option<String>,
    pub github_environment: Option<String>,
    pub worker: Option<String>,
    pub cloudflare_environment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProviderStatus {
    pub id: DeploymentProviderId,
    pub name: &'static str,
    pub available: bool,
    pub detail: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryOptions {
    pub repositories: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryContext {
    pub repository: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubEnvironmentOptions {
    pub repository: String,
    pub environments: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareTargetContext {
    pub worker: Option<String>,
    pub environments: Vec<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPushResult {
    pub provider: DeploymentProviderId,
    pub pushed_count: usize,
    pub failed_keys: Vec<String>,
}

#[derive(Debug)]
pub struct ProviderPushError {
    pub code: &'static str,
    pub message: &'static str,
}

pub fn list(root: &Path) -> Vec<DeploymentProviderStatus> {
    let github_available =
        find_cli("gh", root).is_some_and(|path| command_succeeds(&path, ["--version"]));
    let wrangler_available =
        find_cli("wrangler", root).is_some_and(|path| wrangler_v4_or_later(&path));
    vec![
        DeploymentProviderStatus {
            id: DeploymentProviderId::GithubActions,
            name: "GitHub Actions",
            available: github_available,
            detail: if github_available {
                "GitHub CLI ready"
            } else {
                "GitHub CLI is missing or unavailable"
            },
        },
        DeploymentProviderStatus {
            id: DeploymentProviderId::CloudflareWorkers,
            name: "Cloudflare Workers",
            available: wrangler_available,
            detail: if wrangler_available {
                "Wrangler v4 ready"
            } else {
                "Wrangler v4 is missing or unavailable"
            },
        },
    ]
}

pub fn list_github_repositories(root: &Path) -> Result<GitHubRepositoryOptions, ProviderPushError> {
    let executable = github_cli(root)?;
    let args = [
        OsString::from("api"),
        OsString::from("user/repos"),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from("--paginate"),
        OsString::from("-f"),
        OsString::from("per_page=100"),
        OsString::from("-f"),
        OsString::from("sort=full_name"),
        OsString::from("-f"),
        OsString::from("direction=asc"),
        OsString::from("--jq"),
        OsString::from(".[] | .full_name"),
    ];
    let repositories = run_metadata_lines(&executable, root, &args)?;
    Ok(GitHubRepositoryOptions { repositories })
}

pub fn detect_github_repository(
    root: &Path,
    source_file: &str,
) -> Result<GitHubRepositoryContext, ProviderPushError> {
    let source_directory = source_directory(root, source_file)?;
    let output = background_command("git")
        .arg("-C")
        .arg(&source_directory)
        .args(["remote", "get-url", "origin"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let repository = match output {
        Ok(output) if output.status.success() && output.stdout.len() <= 4096 => {
            String::from_utf8(output.stdout)
                .ok()
                .and_then(|remote| parse_github_remote(&remote))
        }
        _ => None,
    };
    Ok(GitHubRepositoryContext { repository })
}

fn parse_github_remote(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let path = remote
        .strip_prefix("git@github.com:")
        .or_else(|| remote.strip_prefix("https://github.com/"))
        .or_else(|| remote.strip_prefix("http://github.com/"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))
        .or_else(|| remote.strip_prefix("git://github.com/"))?;
    let repository = path
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path);
    validate_repository(repository).ok()?;
    Some(repository.to_owned())
}

pub fn detect_cloudflare_target(
    root: &Path,
    source_file: &str,
) -> Result<CloudflareTargetContext, ProviderPushError> {
    let source_directory = source_directory(root, source_file)?;
    let mut directory = source_directory.as_path();
    loop {
        for name in ["wrangler.jsonc", "wrangler.json", "wrangler.toml"] {
            let config = directory.join(name);
            if config.is_file() {
                return parse_wrangler_config(root, &config);
            }
        }
        if directory == root {
            break;
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        if !parent.starts_with(root) {
            break;
        }
        directory = parent;
    }
    Ok(CloudflareTargetContext {
        worker: None,
        environments: Vec::new(),
        config_path: None,
    })
}

fn source_directory(root: &Path, source_file: &str) -> Result<PathBuf, ProviderPushError> {
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

fn parse_wrangler_config(
    root: &Path,
    config: &Path,
) -> Result<CloudflareTargetContext, ProviderPushError> {
    let bytes = std::fs::read(config).map_err(|_| cloudflare_config_error())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(cloudflare_config_error());
    }
    let content = String::from_utf8(bytes).map_err(|_| cloudflare_config_error())?;
    let (worker, mut environments) = if config.extension().is_some_and(|value| value == "toml") {
        let value =
            toml::from_str::<toml::Value>(&content).map_err(|_| cloudflare_config_error())?;
        let worker = value
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        let environments: Vec<String> = value
            .get("env")
            .and_then(toml::Value::as_table)
            .map(|table| table.keys().cloned().collect())
            .unwrap_or_default();
        (worker, environments)
    } else {
        let value = json5::from_str::<serde_json::Value>(&content)
            .map_err(|_| cloudflare_config_error())?;
        let worker = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let environments: Vec<String> = value
            .get("env")
            .and_then(serde_json::Value::as_object)
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default();
        (worker, environments)
    };
    environments.sort_by_key(|item| item.to_ascii_lowercase());
    environments.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    let config_path = config
        .strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    Ok(CloudflareTargetContext {
        worker,
        environments,
        config_path,
    })
}

pub fn list_github_environments(
    root: &Path,
    repository: &str,
) -> Result<GitHubEnvironmentOptions, ProviderPushError> {
    validate_repository(repository)?;
    let executable = github_cli(root)?;
    let endpoint = format!("repos/{repository}/environments");
    let args = [
        OsString::from("api"),
        OsString::from(endpoint),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from("--paginate"),
        OsString::from("-f"),
        OsString::from("per_page=100"),
        OsString::from("--jq"),
        OsString::from(".environments[] | .name"),
    ];
    let environments = run_metadata_lines(&executable, root, &args)?;
    Ok(GitHubEnvironmentOptions {
        repository: repository.to_owned(),
        environments,
    })
}

pub fn create_github_environment(
    root: &Path,
    repository: &str,
    environment: &str,
) -> Result<GitHubEnvironmentOptions, ProviderPushError> {
    validate_repository(repository)?;
    validate_simple_target(environment)?;
    let executable = github_cli(root)?;
    let endpoint = format!("repos/{repository}/environments/{environment}");
    let args = [
        OsString::from("api"),
        OsString::from("--method"),
        OsString::from("PUT"),
        OsString::from(endpoint),
        OsString::from("--silent"),
    ];
    if !run_metadata_command(&executable, root, &args) {
        return Err(ProviderPushError {
            code: "GITHUB_ENVIRONMENT_CREATE_FAILED",
            message: "GitHub Environment를 만들지 못했습니다. 저장소 관리 권한을 확인해주세요.",
        });
    }
    list_github_environments(root, repository)
}

fn github_cli(root: &Path) -> Result<PathBuf, ProviderPushError> {
    let executable = require_cli("gh", root, "GitHub CLI를 찾지 못했습니다.")?;
    if command_succeeds(&executable, ["--version"]) {
        Ok(executable)
    } else {
        Err(ProviderPushError {
            code: "PROVIDER_CLI_UNSUPPORTED",
            message: "GitHub CLI를 실행할 수 없습니다.",
        })
    }
}

fn run_metadata_lines(
    executable: &Path,
    root: &Path,
    args: &[OsString],
) -> Result<Vec<String>, ProviderPushError> {
    let output = provider_command(executable, args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| provider_metadata_error())?;
    if !output.status.success() || output.stdout.len() > 2 * 1024 * 1024 {
        return Err(provider_metadata_error());
    }
    parse_metadata_lines(output.stdout)
}

fn parse_metadata_lines(stdout: Vec<u8>) -> Result<Vec<String>, ProviderPushError> {
    let mut items = String::from_utf8(stdout)
        .map_err(|_| provider_metadata_error())?
        .lines()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item.to_ascii_lowercase());
    items.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(items)
}

fn run_metadata_command(executable: &Path, root: &Path, args: &[OsString]) -> bool {
    provider_command(executable, args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn provider_metadata_error() -> ProviderPushError {
    ProviderPushError {
        code: "PROVIDER_METADATA_FAILED",
        message: "GitHub 대상 목록을 가져오지 못했습니다. GitHub CLI 로그인을 확인해주세요.",
    }
}

fn cloudflare_config_error() -> ProviderPushError {
    ProviderPushError {
        code: "CLOUDFLARE_CONFIG_FAILED",
        message: "가장 가까운 Wrangler 설정을 읽지 못했습니다. 설정 문법을 확인해주세요.",
    }
}

pub fn push(
    service: &ProjectService,
    request: ProviderPushRequest,
) -> Result<ProviderPushResult, ProviderPushError> {
    let keys = request
        .selections
        .iter()
        .map(|selection| selection.key.clone())
        .collect::<Vec<_>>();
    let values = service
        .provider_values(&request.file, &keys)
        .map_err(ProviderPushError::from)?;
    match request.provider {
        DeploymentProviderId::GithubActions => push_github(service.root(), request, values),
        DeploymentProviderId::CloudflareWorkers => push_cloudflare(service.root(), request, values),
    }
}

fn push_github(
    root: &Path,
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
) -> Result<ProviderPushResult, ProviderPushError> {
    let executable = github_cli(root)?;
    let repository = request.repository.as_deref().ok_or_else(invalid_target)?;
    validate_repository(repository)?;
    let environment = optional_target(request.github_environment.as_deref())?;
    let kinds = request
        .selections
        .iter()
        .map(|selection| (selection.key.as_str(), selection.kind))
        .collect::<BTreeMap<_, _>>();

    let mut pushed_count = 0;
    let mut failed_keys = Vec::new();
    for value in &values {
        let kind = kinds
            .get(value.key())
            .copied()
            .unwrap_or(GitHubEntryKind::Secret);
        let mut args = vec![
            OsString::from(match kind {
                GitHubEntryKind::Secret => "secret",
                GitHubEntryKind::Variable => "variable",
            }),
            OsString::from("set"),
            OsString::from(value.key()),
            OsString::from("--repo"),
            OsString::from(repository),
        ];
        if let Some(environment) = environment {
            args.push(OsString::from("--env"));
            args.push(OsString::from(environment));
        }
        if run_with_stdin(&executable, root, &args, value.value().as_bytes()) {
            pushed_count += 1;
        } else {
            failed_keys.push(value.key().to_owned());
        }
    }
    Ok(ProviderPushResult {
        provider: DeploymentProviderId::GithubActions,
        pushed_count,
        failed_keys,
    })
}

fn push_cloudflare(
    root: &Path,
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
) -> Result<ProviderPushResult, ProviderPushError> {
    if request
        .selections
        .iter()
        .any(|selection| selection.kind != GitHubEntryKind::Secret)
    {
        return Err(invalid_request(
            "Cloudflare Workers에는 현재 Secret만 전송할 수 있습니다.",
        ));
    }
    let executable = require_cli("wrangler", root, "Wrangler v4를 찾지 못했습니다.")?;
    if !wrangler_v4_or_later(&executable) {
        return Err(ProviderPushError {
            code: "PROVIDER_CLI_UNSUPPORTED",
            message: "Cloudflare 전송에는 Wrangler v4 이상이 필요합니다.",
        });
    }
    let worker = request.worker.as_deref().ok_or_else(invalid_target)?;
    validate_simple_target(worker)?;
    let environment = optional_target(request.cloudflare_environment.as_deref())?;
    let mut args = vec![
        OsString::from("secret"),
        OsString::from("bulk"),
        OsString::from("--name"),
        OsString::from(worker),
    ];
    if let Some(environment) = environment {
        args.push(OsString::from("--env"));
        args.push(OsString::from(environment));
    }

    let borrowed = values
        .iter()
        .map(|value| (value.key(), value.value()))
        .collect::<BTreeMap<_, _>>();
    let mut stdin = Zeroizing::new(Vec::new());
    serde_json::to_writer(&mut *stdin, &borrowed).map_err(|_| ProviderPushError {
        code: "PROVIDER_PAYLOAD_FAILED",
        message: "Cloudflare 전송 데이터를 준비하지 못했습니다.",
    })?;
    if !run_with_stdin(&executable, root, &args, &stdin) {
        return Err(ProviderPushError {
            code: "PROVIDER_PUSH_FAILED",
            message: "Cloudflare 전송에 실패했습니다. Wrangler 로그인과 대상을 확인해주세요.",
        });
    }
    Ok(ProviderPushResult {
        provider: DeploymentProviderId::CloudflareWorkers,
        pushed_count: values.len(),
        failed_keys: Vec::new(),
    })
}

fn run_with_stdin(executable: &Path, root: &Path, args: &[OsString], stdin: &[u8]) -> bool {
    let mut command = provider_command(executable, args);
    let mut child = match command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut pipe| pipe.write_all(stdin).is_ok());
    wrote && child.wait().is_ok_and(|status| status.success())
}

fn provider_command(executable: &Path, args: &[OsString]) -> Command {
    let mut command = if cfg!(windows)
        && executable.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        }) {
        let mut command = Command::new("cmd.exe");
        command.arg("/D").arg("/S").arg("/C").arg(executable);
        command.args(args);
        command
    } else {
        let mut command = Command::new(executable);
        command.args(args);
        command
    };
    suppress_console_window(&mut command);
    command
}

fn background_command(executable: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(executable);
    suppress_console_window(&mut command);
    command
}

#[cfg(windows)]
fn suppress_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn suppress_console_window(_command: &mut Command) {}

fn command_succeeds<'a>(executable: &Path, args: impl IntoIterator<Item = &'a str>) -> bool {
    let args = args.into_iter().map(OsString::from).collect::<Vec<_>>();
    provider_command(executable, &args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wrangler_v4_or_later(executable: &Path) -> bool {
    let args = [OsString::from("--version")];
    let output = match provider_command(executable, &args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return false,
    };
    String::from_utf8_lossy(&output.stdout)
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .find_map(|part| part.split('.').next()?.parse::<u64>().ok())
        .is_some_and(|major| major >= 4)
}

fn require_cli(
    name: &str,
    root: &Path,
    message: &'static str,
) -> Result<PathBuf, ProviderPushError> {
    find_cli(name, root).ok_or(ProviderPushError {
        code: "PROVIDER_CLI_NOT_FOUND",
        message,
    })
}

fn find_cli(name: &str, root: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if name == "wrangler" {
        candidates.push(root.join("node_modules/.bin").join(if cfg!(windows) {
            "wrangler.cmd"
        } else {
            "wrangler"
        }));
    }
    let executable_names = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
        ]
    } else {
        vec![name.to_owned()]
    };
    candidates.extend(
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .flat_map(|directory| {
                executable_names
                    .iter()
                    .map(move |executable| directory.join(executable))
            }),
    );
    if let Some(base) = BaseDirs::new() {
        for directory in [
            base.home_dir().join(".local/bin"),
            base.home_dir().join(".cargo/bin"),
        ] {
            for executable in &executable_names {
                candidates.push(directory.join(executable));
            }
        }
        if name == "wrangler" && cfg!(windows) {
            candidates.push(base.home_dir().join("AppData/Roaming/npm/wrangler.cmd"));
        }
    }
    if !cfg!(windows) {
        for executable in &executable_names {
            candidates.push(PathBuf::from("/opt/homebrew/bin").join(executable));
            candidates.push(PathBuf::from("/usr/local/bin").join(executable));
            candidates.push(PathBuf::from("/usr/bin").join(executable));
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn validate_repository(value: &str) -> Result<(), ProviderPushError> {
    let Some((owner, repository)) = value.split_once('/') else {
        return Err(invalid_target());
    };
    if owner.is_empty() || repository.is_empty() || repository.contains('/') {
        return Err(invalid_target());
    }
    validate_simple_target(owner)?;
    validate_simple_target(repository)
}

fn optional_target(value: Option<&str>) -> Result<Option<&str>, ProviderPushError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = value {
        validate_simple_target(value)?;
    }
    Ok(value)
}

fn validate_simple_target(value: &str) -> Result<(), ProviderPushError> {
    let valid = !value.is_empty()
        && value.len() <= 100
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid { Ok(()) } else { Err(invalid_target()) }
}

fn invalid_target() -> ProviderPushError {
    invalid_request("배포 대상 형식이 올바르지 않습니다.")
}

fn invalid_request(message: &'static str) -> ProviderPushError {
    ProviderPushError {
        code: "INVALID_REQUEST",
        message,
    }
}

impl From<EnvError> for ProviderPushError {
    fn from(error: EnvError) -> Self {
        let code = error.code().as_str();
        let code = match code {
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

    #[test]
    fn metadata_lines_are_trimmed_sorted_and_deduplicated() {
        let items = parse_metadata_lines(
            b"zeta/repository\n Alpha/repository \nalpha/repository\n\n".to_vec(),
        )
        .expect("metadata");

        assert_eq!(items, ["Alpha/repository", "zeta/repository"]);
        assert!(parse_metadata_lines(vec![0xff]).is_err());
    }

    #[test]
    fn parses_supported_github_origin_urls() {
        assert_eq!(
            parse_github_remote("git@github.com:owner/repository.git\n").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(
            parse_github_remote("https://github.com/owner/repository.git").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(
            parse_github_remote("ssh://git@github.com/owner/repository").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(
            parse_github_remote("git@gitlab.com:owner/repository.git"),
            None
        );
    }

    #[test]
    fn detects_nearest_wrangler_jsonc_context() {
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(project.path().join("apps/api")).expect("directory");
        std::fs::write(
            project.path().join("wrangler.jsonc"),
            r#"{"name":"root-worker"}"#,
        )
        .expect("root config");
        std::fs::write(
            project.path().join("apps/api/wrangler.jsonc"),
            r#"{
              // JSONC comments and trailing commas are supported.
              "name": "api-worker",
              "env": { "production": {}, "staging": {}, },
            }"#,
        )
        .expect("nearest config");

        let context = detect_cloudflare_target(project.path(), "apps/api/.env").expect("context");
        assert_eq!(context.worker.as_deref(), Some("api-worker"));
        assert_eq!(context.environments, ["production", "staging"]);
        assert_eq!(
            context.config_path.as_deref(),
            Some("apps/api/wrangler.jsonc")
        );
    }

    #[test]
    fn detects_wrangler_toml_context() {
        let project = tempfile::tempdir().expect("project");
        std::fs::write(
            project.path().join("wrangler.toml"),
            "name = \"api-worker\"\n[env.staging]\nname = \"api-worker-staging\"\n[env.production]\nname = \"api-worker-production\"\n",
        )
        .expect("config");

        let context = detect_cloudflare_target(project.path(), ".env").expect("context");
        assert_eq!(context.worker.as_deref(), Some("api-worker"));
        assert_eq!(context.environments, ["production", "staging"]);
    }

    #[test]
    fn cloudflare_payload_is_json_without_debuggable_value_type() {
        let project = tempfile::tempdir().expect("project");
        std::fs::write(
            project.path().join(".env"),
            "API_KEY=fake_secret_canary\nPORT=fake_8787\n",
        )
        .expect("fixture");
        let service = ProjectService::open(project.path()).expect("service");
        service.initialize().expect("initialize");
        let values = service
            .provider_values(".env", &["API_KEY".to_owned(), "PORT".to_owned()])
            .expect("values");
        let borrowed = values
            .iter()
            .map(|value| (value.key(), value.value()))
            .collect::<BTreeMap<_, _>>();
        let mut buffer = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *buffer, &borrowed).expect("serialize");
        assert_eq!(
            String::from_utf8_lossy(&buffer),
            r#"{"API_KEY":"fake_secret_canary","PORT":"fake_8787"}"#
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_cmd_provider_launcher_receives_standard_input() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let launcher = directory.path().join("fake-provider.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nset /p payload=\r\nif \"%payload%\"==\"fake-provider-input\" exit /b 0\r\nexit /b 1\r\n",
        )
        .expect("write launcher");

        assert!(run_with_stdin(
            &launcher,
            directory.path(),
            &[],
            b"fake-provider-input\n",
        ));
    }
}

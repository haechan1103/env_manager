use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use env_core::ProviderValue;

use crate::provider_adapter::{self, AdapterStrategy, ResolvedAdapter};

use super::cli::{background_command, provider_command, run_with_stdin};
use super::error::{ProviderPushError, invalid_target};
use super::model::{
    GITHUB_ACTIONS_ID, GitHubEntryKind, GitHubEnvironmentOptions, GitHubRepositoryContext,
    GitHubRepositoryOptions, OfficialProviderId, ProviderPushRequest, ProviderPushResult,
};
use super::validation::{
    optional_target, source_directory, validate_repository, validate_simple_target,
};

pub fn list_repositories(
    root: &Path,
    app_data: &Path,
) -> Result<GitHubRepositoryOptions, ProviderPushError> {
    let executable = github_cli(root, app_data)?;
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

pub fn detect_repository(
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
                .and_then(|remote| parse_remote(&remote))
        }
        _ => None,
    };
    Ok(GitHubRepositoryContext { repository })
}

pub fn list_environments(
    root: &Path,
    app_data: &Path,
    repository: &str,
) -> Result<GitHubEnvironmentOptions, ProviderPushError> {
    validate_repository(repository)?;
    let executable = github_cli(root, app_data)?;
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

pub fn create_environment(
    root: &Path,
    app_data: &Path,
    repository: &str,
    environment: &str,
) -> Result<GitHubEnvironmentOptions, ProviderPushError> {
    validate_repository(repository)?;
    validate_simple_target(environment)?;
    let executable = github_cli(root, app_data)?;
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
    list_environments(root, app_data, repository)
}

pub(super) fn push(
    root: &Path,
    request: ProviderPushRequest,
    values: Vec<ProviderValue>,
    adapter: ResolvedAdapter,
) -> Result<ProviderPushResult, ProviderPushError> {
    if adapter.strategy != AdapterStrategy::GhSecretSetV1 {
        return Err(ProviderPushError {
            code: "PROVIDER_ADAPTER_INVALID",
            message: "GitHub Adapter 전략이 올바르지 않습니다.",
        });
    }
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
        if run_with_stdin(&adapter.executable, root, &args, value.value().as_bytes()) {
            pushed_count += 1;
        } else {
            failed_keys.push(value.key().to_owned());
        }
    }
    Ok(ProviderPushResult {
        provider: GITHUB_ACTIONS_ID.to_owned(),
        pushed_count,
        failed_keys,
    })
}

fn github_cli(root: &Path, app_data: &Path) -> Result<PathBuf, ProviderPushError> {
    let adapter = provider_adapter::resolve(OfficialProviderId::GithubActions, root, app_data)?;
    if adapter.strategy != AdapterStrategy::GhSecretSetV1 {
        return Err(ProviderPushError {
            code: "PROVIDER_ADAPTER_INVALID",
            message: "GitHub Adapter 전략이 올바르지 않습니다.",
        });
    }
    Ok(adapter.executable)
}

fn parse_remote(remote: &str) -> Option<String> {
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
        .map_err(|_| metadata_error())?;
    if !output.status.success() || output.stdout.len() > 2 * 1024 * 1024 {
        return Err(metadata_error());
    }
    parse_metadata_lines(output.stdout)
}

fn parse_metadata_lines(stdout: Vec<u8>) -> Result<Vec<String>, ProviderPushError> {
    let mut items = String::from_utf8(stdout)
        .map_err(|_| metadata_error())?
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

fn metadata_error() -> ProviderPushError {
    ProviderPushError {
        code: "PROVIDER_METADATA_FAILED",
        message: "GitHub 대상 목록을 가져오지 못했습니다. GitHub CLI 로그인을 확인해주세요.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            parse_remote("git@github.com:owner/repository.git\n").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(
            parse_remote("https://github.com/owner/repository.git").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(
            parse_remote("ssh://git@github.com/owner/repository").as_deref(),
            Some("owner/repository")
        );
        assert_eq!(parse_remote("git@gitlab.com:owner/repository.git"), None);
    }
}

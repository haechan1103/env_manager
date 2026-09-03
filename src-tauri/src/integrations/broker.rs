use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use super::catalog::source_repository_root;
use super::command::{background_command, find_executable};
use super::model::{APP_VERSION, IntegrationError};

fn find_broker() -> Option<PathBuf> {
    if let Some(path) = find_executable("env-manager-broker") {
        return Some(path);
    }
    let base = BaseDirs::new()?;
    let file_name = broker_file_name();
    [
        base.home_dir().join(".cargo/bin").join(file_name),
        base.home_dir().join(".local/bin").join(file_name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

pub(super) fn ensure_current_broker(app: &AppHandle) -> Result<PathBuf, IntegrationError> {
    if let Some(path) = managed_broker(app)
        && broker_version_matches(&path)
    {
        return Ok(path);
    }

    if let Some(bundled) = bundled_broker(app) {
        return install_bundled_broker(app, &bundled);
    }

    // Development fallback. Release builds always carry the broker as a Tauri sidecar.
    if let Some(path) = find_broker()
        && broker_version_matches(&path)
    {
        return Ok(path);
    }
    let cargo = find_executable("cargo").ok_or(IntegrationError {
        code: "BROKER_INSTALL_UNAVAILABLE",
        message: "앱에 포함된 broker를 찾지 못했습니다. 최신 설치본으로 다시 설치해주세요.",
    })?;
    let source = source_repository_root(app).ok_or(IntegrationError {
        code: "BROKER_SOURCE_MISSING",
        message: "broker 소스를 찾지 못했습니다. 앱을 최신 설치본으로 다시 빌드해주세요.",
    })?;
    let mut command = background_command(cargo);
    let success = command
        .args(["install", "--path"])
        .arg(source.join("crates/env-broker"))
        .args(["--locked", "--force"])
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        return Err(IntegrationError {
            code: "BROKER_INSTALL_FAILED",
            message: "로컬 broker를 설치하지 못했습니다.",
        });
    }
    find_broker().ok_or(IntegrationError {
        code: "BROKER_NOT_FOUND",
        message: "broker 설치 후 실행 파일을 찾지 못했습니다.",
    })
}

fn broker_file_name() -> &'static str {
    if cfg!(windows) {
        "env-manager-broker.exe"
    } else {
        "env-manager-broker"
    }
}

fn managed_broker(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|root| {
            root.join("agent-integrations/bin")
                .join(APP_VERSION)
                .join(broker_file_name())
        })
        .filter(|path| path.is_file())
}

fn bundled_broker(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join(broker_file_name()));
    }
    if let Ok(resource) = app
        .path()
        .resolve(broker_file_name(), BaseDirectory::Resource)
    {
        candidates.push(resource);
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file() && broker_version_matches(candidate))
}

fn install_bundled_broker(app: &AppHandle, bundled: &Path) -> Result<PathBuf, IntegrationError> {
    let app_data = app.path().app_data_dir().map_err(|_| IntegrationError {
        code: "APP_DATA_UNAVAILABLE",
        message: "앱 데이터 경로를 확인하지 못했습니다.",
    })?;
    let directory = app_data.join("agent-integrations/bin").join(APP_VERSION);
    fs::create_dir_all(&directory).map_err(|_| IntegrationError {
        code: "BROKER_INSTALL_FAILED",
        message: "broker 설치 디렉터리를 만들지 못했습니다.",
    })?;

    let target = directory.join(broker_file_name());
    let temporary = directory.join(format!(".broker-install-{}", std::process::id()));
    fs::copy(bundled, &temporary).map_err(|_| IntegrationError {
        code: "BROKER_INSTALL_FAILED",
        message: "앱에 포함된 broker를 복사하지 못했습니다.",
    })?;
    set_broker_permissions(&temporary)?;

    if target.exists() {
        fs::remove_file(&target).map_err(|_| IntegrationError {
            code: "BROKER_INSTALL_FAILED",
            message: "이전 broker를 교체하지 못했습니다.",
        })?;
    }
    fs::rename(&temporary, &target).map_err(|_| IntegrationError {
        code: "BROKER_INSTALL_FAILED",
        message: "broker 설치를 완료하지 못했습니다.",
    })?;

    if !broker_version_matches(&target) {
        return Err(IntegrationError {
            code: "BROKER_VERSION_MISMATCH",
            message: "앱과 broker 버전이 일치하지 않습니다. 앱을 다시 설치해주세요.",
        });
    }
    Ok(target)
}

#[cfg(unix)]
fn set_broker_permissions(path: &Path) -> Result<(), IntegrationError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| IntegrationError {
        code: "BROKER_INSTALL_FAILED",
        message: "broker 실행 권한을 설정하지 못했습니다.",
    })
}

#[cfg(not(unix))]
fn set_broker_permissions(_path: &Path) -> Result<(), IntegrationError> {
    Ok(())
}

fn broker_version_matches(path: &Path) -> bool {
    background_command(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| output.split_whitespace().any(|part| part == APP_VERSION))
}

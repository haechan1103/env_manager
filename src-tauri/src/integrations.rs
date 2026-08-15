use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use directories::BaseDirs;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

const PLUGIN_NAME: &str = "env-manager";
const MARKETPLACE_NAME: &str = "env-manager";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const AGENT_BUNDLE_VERSION: &str = include_str!("../../plugins/env-manager/VERSION");

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIntegrationId {
    Codex,
    ClaudeCode,
    GithubCopilot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationStatus {
    pub id: AgentIntegrationId,
    pub name: &'static str,
    pub detected: bool,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub legacy_version: bool,
    pub current_version: &'static str,
    pub update_available: bool,
    pub protection: &'static str,
    pub detail: String,
    pub can_install: bool,
    pub action_blocker: Option<AgentIntegrationBlocker>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIntegrationBlocker {
    ToolNotFound,
    BrokerUnavailable,
    BundleUnavailable,
}

#[derive(Debug)]
pub struct IntegrationError {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallationMarker {
    #[serde(alias = "version")]
    bundle_version: String,
}

pub fn list(app: &AppHandle) -> Vec<AgentIntegrationStatus> {
    let broker_available = ensure_current_broker(app).is_ok();
    let catalog_available = catalog_source(app).is_some();
    [
        AgentIntegrationId::Codex,
        AgentIntegrationId::ClaudeCode,
        AgentIntegrationId::GithubCopilot,
    ]
    .into_iter()
    .map(|id| status(app, id, broker_available, catalog_available))
    .collect()
}

pub fn install(
    app: &AppHandle,
    id: AgentIntegrationId,
) -> Result<AgentIntegrationStatus, IntegrationError> {
    let executable = integration_executable(id).ok_or(IntegrationError {
        code: "AGENT_NOT_FOUND",
        message: "먼저 해당 AI 코딩 도구를 설치해주세요.",
    })?;
    let broker = ensure_current_broker(app)?;
    let catalog = materialize_catalog(app, &broker, id)?;

    let _ = run_agent_command(
        &executable,
        marketplace_add_args(id, catalog.as_os_str().to_owned()),
    );
    let command_succeeded = install_or_update(&executable, id);
    if !command_succeeded {
        return Err(IntegrationError {
            code: "AGENT_INSTALL_FAILED",
            message: "플러그인을 설치하지 못했습니다. 해당 도구의 로그인과 플러그인 정책을 확인해주세요.",
        });
    }

    if !current_bundle_is_cached(id) && marker_version(app, id).is_some() {
        reconnect_owned_marketplace(&executable, id, catalog.as_os_str().to_owned())?;
        if !install_or_update(&executable, id) {
            return Err(IntegrationError {
                code: "AGENT_INSTALL_FAILED",
                message: "플러그인을 설치하지 못했습니다. 해당 도구의 로그인과 플러그인 정책을 확인해주세요.",
            });
        }
    }
    if !current_bundle_is_cached(id) {
        return Err(IntegrationError {
            code: "AGENT_BUNDLE_NOT_UPDATED",
            message: "AI 도구가 새 연동 번들을 적용하지 않았습니다. 기존 marketplace 연결을 확인해주세요.",
        });
    }

    persist_marker(app, id)?;
    Ok(status(app, id, true, true))
}

fn status(
    app: &AppHandle,
    id: AgentIntegrationId,
    broker_available: bool,
    catalog_available: bool,
) -> AgentIntegrationStatus {
    let cli_detected = integration_executable(id).is_some();
    let vscode_detected = id == AgentIntegrationId::GithubCopilot && detect_vscode();
    let detected = cli_detected || vscode_detected;
    let installed_version = installed_version(app, id);
    let installed = installed_version.is_some();
    let legacy_version = installed_version
        .as_deref()
        .is_some_and(is_legacy_bundle_version);
    let update_available = installed_version
        .as_deref()
        .is_some_and(|version| is_update_available(version, agent_bundle_version()));
    let action_blocker = if !cli_detected {
        Some(AgentIntegrationBlocker::ToolNotFound)
    } else if !broker_available {
        Some(AgentIntegrationBlocker::BrokerUnavailable)
    } else if !catalog_available {
        Some(AgentIntegrationBlocker::BundleUnavailable)
    } else {
        None
    };
    let (protection, detail) = match (id, installed, cli_detected, vscode_detected) {
        (AgentIntegrationId::Codex, true, _, _) => (
            "broker",
            "Redacted broker가 연결되어 있습니다. 직접 파일 차단 수준은 Codex 권한 프로필에 따라 달라집니다.".to_owned(),
        ),
        (AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot, true, _, _) => (
            "guarded",
            "공통 Skill, MCP broker, 직접 env 접근 Guard가 연결되어 있습니다.".to_owned(),
        ),
        (AgentIntegrationId::GithubCopilot, false, false, true) => (
            "inactive",
            "VS Code는 감지했지만 Copilot CLI가 필요합니다. CLI 설치 후 여기서 한 번에 연결할 수 있습니다.".to_owned(),
        ),
        (_, false, true, _) => (
            "inactive",
            "도구를 감지했습니다. Env Manager 연동을 설치할 수 있습니다.".to_owned(),
        ),
        _ => (
            "inactive",
            "도구가 설치되면 Env Manager에서 연동할 수 있습니다.".to_owned(),
        ),
    };

    AgentIntegrationStatus {
        id,
        name: integration_name(id),
        detected,
        installed,
        installed_version,
        legacy_version,
        current_version: agent_bundle_version(),
        update_available,
        protection,
        detail,
        can_install: action_blocker.is_none(),
        action_blocker,
    }
}

fn agent_bundle_version() -> &'static str {
    AGENT_BUNDLE_VERSION.trim()
}

fn is_update_available(installed: &str, current: &str) -> bool {
    match (Version::parse(installed), Version::parse(current)) {
        (Ok(installed), Ok(current)) => installed < current,
        _ => installed != current,
    }
}

fn is_legacy_bundle_version(version: &str) -> bool {
    Version::parse(version).is_ok_and(|version| version.major == 0)
}

fn integration_name(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => "Codex",
        AgentIntegrationId::ClaudeCode => "Claude Code",
        AgentIntegrationId::GithubCopilot => "GitHub Copilot / VS Code",
    }
}

fn integration_executable(id: AgentIntegrationId) -> Option<PathBuf> {
    executable_candidates(id).into_iter().find(|executable| {
        agent_command(executable, &[OsString::from("--version")])
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

fn executable_candidates(id: AgentIntegrationId) -> Vec<PathBuf> {
    let name = match id {
        AgentIntegrationId::Codex => "codex",
        AgentIntegrationId::ClaudeCode => "claude",
        AgentIntegrationId::GithubCopilot => "copilot",
    };
    let mut candidates = executable_candidates_named(name);
    if cfg!(target_os = "macos") {
        match id {
            AgentIntegrationId::Codex => candidates.push(PathBuf::from(
                "/Applications/Codex.app/Contents/Resources/codex",
            )),
            AgentIntegrationId::ClaudeCode => {}
            AgentIntegrationId::GithubCopilot => {}
        }
    }
    deduplicate_paths(candidates)
}

fn detect_vscode() -> bool {
    if find_executable("code").is_some() {
        return true;
    }
    if cfg!(target_os = "macos") {
        return [
            "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
            "/Applications/Visual Studio Code - Insiders.app/Contents/Resources/app/bin/code",
        ]
        .iter()
        .any(|path| Path::new(path).is_file());
    }
    if cfg!(windows)
        && let Some(base) = BaseDirs::new()
    {
        return [
            base.data_local_dir()
                .join("Programs/Microsoft VS Code/bin/code.cmd"),
            base.data_local_dir()
                .join("Programs/Microsoft VS Code Insiders/bin/code-insiders.cmd"),
        ]
        .iter()
        .any(|path| path.is_file());
    }
    false
}

fn find_executable(name: &str) -> Option<PathBuf> {
    executable_candidates_named(name).into_iter().next()
}

fn executable_candidates_named(name: &str) -> Vec<PathBuf> {
    let file_names = executable_file_names(name, cfg!(windows));
    let mut candidates = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .flat_map(|directory| {
            file_names
                .iter()
                .map(move |file_name| directory.join(file_name))
        })
        .filter(|candidate| candidate.is_file())
        .collect::<Vec<_>>();

    if let Some(base) = BaseDirs::new() {
        for directory in [
            base.home_dir().join(".local/bin"),
            base.home_dir().join(".cargo/bin"),
            base.home_dir().join(".npm-global/bin"),
            base.home_dir().join(".bun/bin"),
            base.home_dir().join("Library/pnpm"),
        ] {
            for file_name in &file_names {
                let candidate = directory.join(file_name);
                if candidate.is_file() {
                    candidates.push(candidate);
                }
            }
        }
        if cfg!(windows) {
            for directory in [
                base.data_dir().join("npm"),
                base.data_local_dir().join("pnpm"),
            ] {
                for file_name in &file_names {
                    let candidate = directory.join(file_name);
                    if candidate.is_file() {
                        candidates.push(candidate);
                    }
                }
            }
        }
    }
    if cfg!(target_os = "macos") {
        for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            for file_name in &file_names {
                let candidate = Path::new(directory).join(file_name);
                if candidate.is_file() {
                    candidates.push(candidate);
                }
            }
        }
    }
    deduplicate_paths(candidates)
}

fn executable_file_names(name: &str, windows: bool) -> Vec<String> {
    if windows {
        ["exe", "cmd", "bat"]
            .into_iter()
            .map(|extension| format!("{name}.{extension}"))
            .collect()
    } else {
        vec![name.to_owned()]
    }
}

fn deduplicate_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().fold(Vec::new(), |mut unique, path| {
        if !unique.contains(&path) {
            unique.push(path);
        }
        unique
    })
}

fn find_broker() -> Option<PathBuf> {
    if let Some(path) = find_executable("env-manager-broker") {
        return Some(path);
    }
    let base = BaseDirs::new()?;
    let file_name = if cfg!(windows) {
        "env-manager-broker.exe"
    } else {
        "env-manager-broker"
    };
    [
        base.home_dir().join(".cargo/bin").join(file_name),
        base.home_dir().join(".local/bin").join(file_name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

fn ensure_current_broker(app: &AppHandle) -> Result<PathBuf, IntegrationError> {
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
    let broker_crate = source.join("crates/env-broker");
    let mut command = background_command(cargo);
    let success = command
        .args(["install", "--path"])
        .arg(&broker_crate)
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

fn catalog_source(app: &AppHandle) -> Option<PathBuf> {
    let bundled = app
        .path()
        .resolve("agent-integrations/catalog", BaseDirectory::Resource)
        .ok();
    if bundled.as_ref().is_some_and(|path| catalog_is_valid(path)) {
        return bundled;
    }
    source_repository_root(app).filter(|path| catalog_is_valid(path))
}

fn source_repository_root(_app: &AppHandle) -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .filter(|path| path.join("crates/env-broker/Cargo.toml").is_file())
}

fn catalog_is_valid(root: &Path) -> bool {
    let plugin = root.join("plugins/env-manager");
    let codex_manifest = plugin.join(".codex-plugin/plugin.json");
    let claude_manifest = plugin.join(".claude-plugin/plugin.json");
    let marketplace = root.join(".claude-plugin/marketplace.json");
    plugin.join("VERSION").is_file()
        && root.join(".agents/plugins/marketplace.json").is_file()
        && manifest_version(&codex_manifest).as_deref() == Some(agent_bundle_version())
        && manifest_version(&claude_manifest).as_deref() == Some(agent_bundle_version())
        && marketplace_version(&marketplace).as_deref() == Some(agent_bundle_version())
}

fn materialize_catalog(
    app: &AppHandle,
    broker: &Path,
    id: AgentIntegrationId,
) -> Result<PathBuf, IntegrationError> {
    let source = catalog_source(app).ok_or(IntegrationError {
        code: "PLUGIN_CATALOG_MISSING",
        message: "앱에 포함된 플러그인 catalog를 찾지 못했습니다.",
    })?;
    let app_data = app.path().app_data_dir().map_err(|_| IntegrationError {
        code: "APP_DATA_UNAVAILABLE",
        message: "앱 데이터 경로를 확인하지 못했습니다.",
    })?;
    let target = app_data
        .join("agent-integrations/catalogs")
        .join(agent_bundle_version())
        .join(integration_slug(id));
    copy_directory(&source.join("plugins"), &target.join("plugins"))?;
    copy_directory(&source.join(".agents"), &target.join(".agents"))?;
    copy_directory(
        &source.join(".claude-plugin"),
        &target.join(".claude-plugin"),
    )?;

    let plugin = target.join("plugins/env-manager");
    let mcp_path = plugin.join(".mcp.json");
    let mut mcp = read_json(&mcp_path)?;
    mcp["mcpServers"]["env-manager"]["command"] =
        Value::String(broker.to_string_lossy().into_owned());
    mcp["mcpServers"]["env-manager"]["env"] = json!({
        "ENV_MANAGER_AUDIT_DIR": app_data.join("agent-activity").to_string_lossy(),
        "ENV_MANAGER_AGENT_HOST": integration_slug(id),
    });
    write_json(&mcp_path, &mcp)?;

    let hook_path = plugin.join("hooks/hooks.json");
    let mut hooks = read_json(&hook_path)?;
    let quoted_broker = format!("\"{}\" guard-hook", broker.to_string_lossy());
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"] = Value::String(quoted_broker);
    write_json(&hook_path, &hooks)?;
    Ok(target)
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), IntegrationError> {
    fs::create_dir_all(target).map_err(|_| IntegrationError {
        code: "PLUGIN_COPY_FAILED",
        message: "플러그인 설치 디렉터리를 만들지 못했습니다.",
    })?;
    let entries = fs::read_dir(source).map_err(|_| IntegrationError {
        code: "PLUGIN_SOURCE_UNAVAILABLE",
        message: "플러그인 원본을 읽지 못했습니다.",
    })?;
    for entry in entries {
        let entry = entry.map_err(|_| IntegrationError {
            code: "PLUGIN_SOURCE_UNAVAILABLE",
            message: "플러그인 원본을 읽지 못했습니다.",
        })?;
        let file_type = entry.file_type().map_err(|_| IntegrationError {
            code: "PLUGIN_SOURCE_UNAVAILABLE",
            message: "플러그인 파일 형식을 확인하지 못했습니다.",
        })?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination).map_err(|_| IntegrationError {
                code: "PLUGIN_COPY_FAILED",
                message: "플러그인 파일을 복사하지 못했습니다.",
            })?;
        } else {
            return Err(IntegrationError {
                code: "PLUGIN_SOURCE_UNSUPPORTED",
                message: "플러그인 원본에 지원하지 않는 파일 형식이 있습니다.",
            });
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, IntegrationError> {
    let bytes = fs::read(path).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_UNAVAILABLE",
        message: "플러그인 설정을 읽지 못했습니다.",
    })?;
    serde_json::from_slice(&bytes).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_INVALID",
        message: "플러그인 설정 형식이 올바르지 않습니다.",
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), IntegrationError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_INVALID",
        message: "플러그인 설정 형식이 올바르지 않습니다.",
    })?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_WRITE_FAILED",
        message: "플러그인 설정을 저장하지 못했습니다.",
    })
}

fn marketplace_add_args(id: AgentIntegrationId, catalog: OsString) -> Vec<OsString> {
    let _ = id;
    vec!["plugin".into(), "marketplace".into(), "add".into(), catalog]
}

fn marketplace_remove_args() -> Vec<OsString> {
    vec![
        "plugin".into(),
        "marketplace".into(),
        "remove".into(),
        MARKETPLACE_NAME.into(),
    ]
}

fn reconnect_owned_marketplace(
    executable: &Path,
    id: AgentIntegrationId,
    catalog: OsString,
) -> Result<(), IntegrationError> {
    // Only call this after finding an app-owned installation marker. A user-owned
    // marketplace with the same name must never be removed automatically.
    let _ = run_agent_command(executable, marketplace_remove_args());
    if run_agent_command(executable, marketplace_add_args(id, catalog)) {
        Ok(())
    } else {
        Err(IntegrationError {
            code: "AGENT_MARKETPLACE_FAILED",
            message: "AI 도구의 Env Manager marketplace를 연결하지 못했습니다.",
        })
    }
}

fn install_or_update(executable: &Path, id: AgentIntegrationId) -> bool {
    run_agent_command(executable, install_args(id))
        || run_agent_command(executable, update_args(id))
}

fn current_bundle_is_cached(id: AgentIntegrationId) -> bool {
    cache_version(id).as_deref() == Some(agent_bundle_version())
}

fn install_args(id: AgentIntegrationId) -> Vec<OsString> {
    let plugin = format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}");
    match id {
        AgentIntegrationId::Codex => vec!["plugin".into(), "add".into(), plugin.into()],
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            vec!["plugin".into(), "install".into(), plugin.into()]
        }
    }
}

fn update_args(id: AgentIntegrationId) -> Vec<OsString> {
    let plugin = format!("{PLUGIN_NAME}@{MARKETPLACE_NAME}");
    match id {
        AgentIntegrationId::Codex => vec!["plugin".into(), "add".into(), plugin.into()],
        AgentIntegrationId::ClaudeCode | AgentIntegrationId::GithubCopilot => {
            vec!["plugin".into(), "update".into(), plugin.into()]
        }
    }
}

fn run_agent_command(executable: &Path, args: Vec<OsString>) -> bool {
    agent_command(executable, &args)
        .status()
        .is_ok_and(|status| status.success())
}

fn agent_command(executable: &Path, args: &[OsString]) -> Command {
    // Rust applies Windows batch-file escaping when the program itself is a
    // `.cmd`/`.bat` path. Avoid constructing a `cmd.exe /C` command line here:
    // catalog paths can contain shell metacharacters and must remain literal args.
    let mut command = background_command(executable);
    command.args(args);
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

fn persist_marker(app: &AppHandle, id: AgentIntegrationId) -> Result<(), IntegrationError> {
    let app_data = app.path().app_data_dir().map_err(|_| IntegrationError {
        code: "APP_DATA_UNAVAILABLE",
        message: "앱 데이터 경로를 확인하지 못했습니다.",
    })?;
    let directory = app_data.join("agent-integrations/installations");
    fs::create_dir_all(&directory).map_err(|_| IntegrationError {
        code: "INSTALL_STATE_WRITE_FAILED",
        message: "연동 설치 상태를 저장하지 못했습니다.",
    })?;
    write_json(
        &directory.join(format!("{}.json", integration_slug(id))),
        &json!(InstallationMarker {
            bundle_version: agent_bundle_version().to_owned(),
        }),
    )
}

fn installed_version(app: &AppHandle, id: AgentIntegrationId) -> Option<String> {
    cache_version(id).or_else(|| marker_version(app, id))
}

fn marker_version(app: &AppHandle, id: AgentIntegrationId) -> Option<String> {
    let path = app
        .path()
        .app_data_dir()
        .ok()?
        .join("agent-integrations/installations")
        .join(format!("{}.json", integration_slug(id)));
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<InstallationMarker>(&bytes)
        .ok()
        .map(|marker| marker.bundle_version)
}

fn cache_version(id: AgentIntegrationId) -> Option<String> {
    let base = BaseDirs::new()?;
    let (root, manifest) = match id {
        AgentIntegrationId::Codex => (
            base.home_dir()
                .join(".codex/plugins/cache/env-manager/env-manager"),
            ".codex-plugin/plugin.json",
        ),
        AgentIntegrationId::ClaudeCode => (
            base.home_dir()
                .join(".claude/plugins/cache/env-manager/env-manager"),
            ".claude-plugin/plugin.json",
        ),
        AgentIntegrationId::GithubCopilot => (
            base.home_dir()
                .join(".copilot/installed-plugins/env-manager/env-manager"),
            ".claude-plugin/plugin.json",
        ),
    };
    newest_manifest_version(&root, manifest)
}

fn newest_manifest_version(root: &Path, manifest: &str) -> Option<String> {
    let entries = fs::read_dir(root).ok()?;
    let versions = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(manifest).is_file())
        .filter_map(|entry| {
            let bytes = fs::read(entry.path().join(manifest)).ok()?;
            serde_json::from_slice::<Value>(&bytes)
                .ok()?
                .get("version")?
                .as_str()
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    versions.into_iter().max_by(
        |left, right| match (Version::parse(left), Version::parse(right)) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
    )
}

fn manifest_version(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

fn marketplace_version(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("plugins")?
        .as_array()?
        .iter()
        .find(|plugin| plugin.get("name").and_then(Value::as_str) == Some(PLUGIN_NAME))?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

fn integration_slug(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => "codex",
        AgentIntegrationId::ClaudeCode => "claude-code",
        AgentIntegrationId::GithubCopilot => "github-copilot",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_integration_uses_the_shared_marketplace_plugin_name() {
        for id in [
            AgentIntegrationId::Codex,
            AgentIntegrationId::ClaudeCode,
            AgentIntegrationId::GithubCopilot,
        ] {
            let install = install_args(id);
            assert!(install.iter().any(|arg| arg == "env-manager@env-manager"));
        }
    }

    #[test]
    fn catalog_validation_requires_both_agent_manifests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        assert!(catalog_is_valid(root));
    }

    #[test]
    fn agent_bundle_version_is_independent_from_the_app_release() {
        assert_eq!(agent_bundle_version(), "1.0.0");
    }

    #[test]
    fn update_detection_uses_semantic_precedence_without_downgrading() {
        assert!(is_update_available("0.5.0", "1.0.0"));
        assert!(!is_update_available("1.0.0", "1.0.0"));
        assert!(!is_update_available("1.1.0", "1.0.0"));
        assert!(!is_update_available("1.0.0+codex.local", "1.0.0"));
    }

    #[test]
    fn legacy_installation_markers_remain_readable() {
        let marker = serde_json::from_str::<InstallationMarker>(r#"{"version":"0.5.0"}"#)
            .expect("legacy marker");
        assert_eq!(marker.bundle_version, "0.5.0");
        assert!(is_legacy_bundle_version(&marker.bundle_version));
    }

    #[test]
    fn windows_agent_cli_candidates_include_native_and_script_launchers() {
        assert_eq!(
            executable_file_names("codex", true),
            vec!["codex.exe", "codex.cmd", "codex.bat"]
        );
        assert_eq!(executable_file_names("codex", false), vec!["codex"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_cmd_agent_launcher_executes_with_literal_arguments() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let launcher = directory.path().join("fake-agent.cmd");
        fs::write(
            &launcher,
            "@echo off\r\nif \"%~1\"==\"--version\" exit /b 0\r\nexit /b 1\r\n",
        )
        .expect("write launcher");

        let status = agent_command(&launcher, &[OsString::from("--version")])
            .status()
            .expect("run launcher");

        assert!(status.success());
    }
}

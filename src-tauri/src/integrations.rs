use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

const PLUGIN_NAME: &str = "env-manager";
const MARKETPLACE_NAME: &str = "env-manager";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    pub current_version: &'static str,
    pub update_available: bool,
    pub protection: &'static str,
    pub detail: String,
    pub can_install: bool,
}

#[derive(Debug)]
pub struct IntegrationError {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallationMarker {
    version: String,
}

pub fn list(app: &AppHandle) -> Vec<AgentIntegrationStatus> {
    let broker_available = find_broker().is_some() || find_executable("cargo").is_some();
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
    let catalog = materialize_catalog(app, &broker)?;

    let _ = run_agent_command(
        &executable,
        marketplace_add_args(id, catalog.as_os_str().to_owned()),
    );
    let installed = run_agent_command(&executable, install_args(id));
    let updated = if installed {
        true
    } else {
        run_agent_command(&executable, update_args(id))
    };
    if !updated {
        return Err(IntegrationError {
            code: "AGENT_INSTALL_FAILED",
            message: "플러그인을 설치하지 못했습니다. 해당 도구의 로그인과 플러그인 정책을 확인해주세요.",
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
    let update_available = installed_version
        .as_deref()
        .is_some_and(|version| version != CURRENT_VERSION);
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
        current_version: CURRENT_VERSION,
        update_available,
        protection,
        detail,
        can_install: cli_detected && broker_available && catalog_available,
    }
}

fn integration_name(id: AgentIntegrationId) -> &'static str {
    match id {
        AgentIntegrationId::Codex => "Codex",
        AgentIntegrationId::ClaudeCode => "Claude Code",
        AgentIntegrationId::GithubCopilot => "GitHub Copilot / VS Code",
    }
}

fn integration_executable(id: AgentIntegrationId) -> Option<PathBuf> {
    let executable = match id {
        AgentIntegrationId::Codex => find_executable("codex"),
        AgentIntegrationId::ClaudeCode => find_executable("claude"),
        AgentIntegrationId::GithubCopilot => find_executable("copilot"),
    }?;
    Command::new(&executable)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| executable)
}

fn detect_vscode() -> bool {
    find_executable("code").is_some()
        || [
            "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
            "/Applications/Visual Studio Code - Insiders.app/Contents/Resources/app/bin/code",
        ]
        .iter()
        .any(|path| Path::new(path).is_file())
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(&file_name))
        .find(|candidate| candidate.is_file())
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
    if let Some(path) = find_broker()
        && broker_version_matches(&path)
    {
        return Ok(path);
    }
    let cargo = find_executable("cargo").ok_or(IntegrationError {
        code: "BROKER_INSTALL_UNAVAILABLE",
        message: "Rust가 없어 broker를 설치할 수 없습니다. 현재 Preview에서는 Rust 설치가 필요합니다.",
    })?;
    let source = source_repository_root(app).ok_or(IntegrationError {
        code: "BROKER_SOURCE_MISSING",
        message: "broker 소스를 찾지 못했습니다. 앱을 최신 설치본으로 다시 빌드해주세요.",
    })?;
    let broker_crate = source.join("crates/env-broker");
    let success = Command::new(cargo)
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

fn broker_version_matches(path: &Path) -> bool {
    Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| {
            output
                .split_whitespace()
                .any(|part| part == CURRENT_VERSION)
        })
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
    root.join("plugins/env-manager/.codex-plugin/plugin.json")
        .is_file()
        && root
            .join("plugins/env-manager/.claude-plugin/plugin.json")
            .is_file()
        && root.join(".agents/plugins/marketplace.json").is_file()
        && root.join(".claude-plugin/marketplace.json").is_file()
}

fn materialize_catalog(app: &AppHandle, broker: &Path) -> Result<PathBuf, IntegrationError> {
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
        .join(CURRENT_VERSION);
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
    Command::new(executable)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

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
            version: CURRENT_VERSION.to_owned(),
        }),
    )
}

fn installed_version(app: &AppHandle, id: AgentIntegrationId) -> Option<String> {
    marker_version(app, id).or_else(|| cache_version(id))
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
        .map(|marker| marker.version)
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
    let mut versions = entries
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
    versions.sort();
    versions.pop()
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
}

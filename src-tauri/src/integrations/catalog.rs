use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use super::model::{
    AgentIntegrationId, IntegrationError, PLUGIN_NAME, agent_bundle_version, integration_slug,
    marketplace_name,
};

pub(super) fn catalog_source(app: &AppHandle) -> Option<PathBuf> {
    let bundled = app
        .path()
        .resolve("agent-integrations/catalog", BaseDirectory::Resource)
        .ok();
    if bundled.as_ref().is_some_and(|path| catalog_is_valid(path)) {
        return bundled;
    }
    source_repository_root(app).filter(|path| catalog_is_valid(path))
}

pub(super) fn source_repository_root(_app: &AppHandle) -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .filter(|path| path.join("crates/env-broker/Cargo.toml").is_file())
}

pub(super) fn catalog_is_valid(root: &Path) -> bool {
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

pub(super) fn materialize_catalog(
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
    if id == AgentIntegrationId::Codex {
        rewrite_marketplace_name(
            &target.join(".agents/plugins/marketplace.json"),
            marketplace_name(id),
        )?;
    }

    let plugin = target.join("plugins/env-manager");
    let mcp_path = plugin.join(".mcp.json");
    let mut mcp = read_json(&mcp_path)?;
    mcp["mcpServers"]["env-manager"]["command"] =
        Value::String(broker.to_string_lossy().into_owned());
    mcp["mcpServers"]["env-manager"]["env"] = json!({
        "ENV_MANAGER_AUDIT_DIR": app_data.join("agent-activity").to_string_lossy(),
        "ENV_MANAGER_APP_DATA_DIR": app_data.to_string_lossy(),
        "ENV_MANAGER_AGENT_HOST": integration_slug(id),
    });
    write_json(&mcp_path, &mcp)?;

    let hook_path = plugin.join("hooks/hooks.json");
    let mut hooks = read_json(&hook_path)?;
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"] =
        Value::String(format!("\"{}\" guard-hook", broker.to_string_lossy()));
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

pub(super) fn read_json(path: &Path) -> Result<Value, IntegrationError> {
    let bytes = fs::read(path).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_UNAVAILABLE",
        message: "플러그인 설정을 읽지 못했습니다.",
    })?;
    serde_json::from_slice(&bytes).map_err(|_| IntegrationError {
        code: "PLUGIN_CONFIG_INVALID",
        message: "플러그인 설정 형식이 올바르지 않습니다.",
    })
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), IntegrationError> {
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

pub(super) fn rewrite_marketplace_name(path: &Path, name: &str) -> Result<(), IntegrationError> {
    let mut marketplace = read_json(path)?;
    marketplace["name"] = Value::String(name.to_owned());
    write_json(path, &marketplace)
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

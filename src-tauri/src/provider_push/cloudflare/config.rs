use std::collections::BTreeMap;
use std::path::Path;

use super::super::error::ProviderPushError;
use super::super::model::CloudflareTargetContext;
use super::super::validation::source_directory;

pub fn detect_target(
    root: &Path,
    source_file: &str,
) -> Result<CloudflareTargetContext, ProviderPushError> {
    let source_directory = source_directory(root, source_file)?;
    let mut directory = source_directory.as_path();
    loop {
        for name in ["wrangler.jsonc", "wrangler.json", "wrangler.toml"] {
            let config = directory.join(name);
            if config.is_file() {
                return parse(root, &config);
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
        account_id: None,
        environment_account_ids: BTreeMap::new(),
    })
}

fn parse(root: &Path, config: &Path) -> Result<CloudflareTargetContext, ProviderPushError> {
    let bytes = std::fs::read(config).map_err(|_| config_error())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(config_error());
    }
    let content = String::from_utf8(bytes).map_err(|_| config_error())?;
    let (worker, mut environments, account_id, environment_account_ids) =
        if config.extension().is_some_and(|value| value == "toml") {
            parse_toml(&content)?
        } else {
            parse_json(&content)?
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
        account_id,
        environment_account_ids,
    })
}

type ParsedConfig = (
    Option<String>,
    Vec<String>,
    Option<String>,
    BTreeMap<String, String>,
);

fn parse_toml(content: &str) -> Result<ParsedConfig, ProviderPushError> {
    let value = toml::from_str::<toml::Value>(content).map_err(|_| config_error())?;
    let worker = value
        .get("name")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let environments = value
        .get("env")
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default();
    let account_id = value
        .get("account_id")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let environment_account_ids = value
        .get("env")
        .and_then(toml::Value::as_table)
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, environment)| {
                    environment
                        .get("account_id")
                        .and_then(toml::Value::as_str)
                        .map(|account| (name.clone(), account.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((worker, environments, account_id, environment_account_ids))
}

fn parse_json(content: &str) -> Result<ParsedConfig, ProviderPushError> {
    let value = json5::from_str::<serde_json::Value>(content).map_err(|_| config_error())?;
    let worker = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let environments = value
        .get("env")
        .and_then(serde_json::Value::as_object)
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    let account_id = value
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let environment_account_ids = value
        .get("env")
        .and_then(serde_json::Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(name, environment)| {
                    environment
                        .get("account_id")
                        .and_then(serde_json::Value::as_str)
                        .map(|account| (name.clone(), account.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((worker, environments, account_id, environment_account_ids))
}

fn config_error() -> ProviderPushError {
    ProviderPushError {
        code: "CLOUDFLARE_CONFIG_FAILED",
        message: "가장 가까운 Wrangler 설정을 읽지 못했습니다. 설정 문법을 확인해주세요.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
              "account_id": "account-default",
              "env": {
                "production": {},
                "staging": { "account_id": "account-staging" },
              },
            }"#,
        )
        .expect("nearest config");

        let context = detect_target(project.path(), "apps/api/.env").expect("context");
        assert_eq!(context.worker.as_deref(), Some("api-worker"));
        assert_eq!(context.environments, ["production", "staging"]);
        assert_eq!(
            context.config_path.as_deref(),
            Some("apps/api/wrangler.jsonc")
        );
        assert_eq!(context.account_id.as_deref(), Some("account-default"));
        assert_eq!(
            context
                .environment_account_ids
                .get("staging")
                .map(String::as_str),
            Some("account-staging")
        );
    }

    #[test]
    fn detects_wrangler_toml_context() {
        let project = tempfile::tempdir().expect("project");
        std::fs::write(
            project.path().join("wrangler.toml"),
            "name = \"api-worker\"\naccount_id = \"account-default\"\n[env.staging]\nname = \"api-worker-staging\"\naccount_id = \"account-staging\"\n[env.production]\nname = \"api-worker-production\"\n",
        )
        .expect("config");

        let context = detect_target(project.path(), ".env").expect("context");
        assert_eq!(context.worker.as_deref(), Some("api-worker"));
        assert_eq!(context.environments, ["production", "staging"]);
        assert_eq!(context.account_id.as_deref(), Some("account-default"));
        assert_eq!(
            context
                .environment_account_ids
                .get("staging")
                .map(String::as_str),
            Some("account-staging")
        );
    }
}

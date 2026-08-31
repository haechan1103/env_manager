use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use env_core::{EnvError, EnvResult};
use env_team::TeamChannelRegistration;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistration {
    pub id: String,
    pub name: String,
    pub display_path: String,
    pub root: PathBuf,
    #[serde(default)]
    pub file_labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderPushReceipt {
    pub timestamp_ms: u64,
    pub project_id: String,
    pub provider: String,
    pub source_file: String,
    pub destination: String,
    pub succeeded_keys: Vec<String>,
    pub failed_keys: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryData {
    #[serde(default)]
    pub projects: Vec<ProjectRegistration>,
    #[serde(default)]
    pub last_selected_project_id: Option<String>,
    #[serde(default)]
    #[serde(deserialize_with = "env_team::deserialize_team_channel_registrations")]
    pub team_channels: Vec<TeamChannelRegistration>,
    #[serde(default)]
    pub provider_push_receipts: Vec<ProviderPushReceipt>,
}

pub fn read(path: &Path) -> EnvResult<RegistryData> {
    with_lock(path, false, || read_unlocked(path))
}

pub fn write(path: &Path, registry: &RegistryData) -> EnvResult<()> {
    with_lock(path, true, || write_unlocked(path, registry))
}

pub fn update<R>(
    path: &Path,
    operation: impl FnOnce(&mut RegistryData) -> EnvResult<R>,
) -> EnvResult<(RegistryData, R)> {
    with_lock(path, true, || {
        let mut registry = read_unlocked(path)?;
        let result = operation(&mut registry)?;
        write_unlocked(path, &registry)?;
        Ok((registry, result))
    })
}

fn with_lock<R>(
    path: &Path,
    exclusive: bool,
    operation: impl FnOnce() -> EnvResult<R>,
) -> EnvResult<R> {
    let parent = path
        .parent()
        .ok_or_else(|| EnvError::invalid("앱 상태 경로가 올바르지 않습니다."))?;
    fs::create_dir_all(parent).map_err(|error| EnvError::io(parent, error))?;
    let lock_path = parent.join("projects.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| EnvError::io(&lock_path, error))?;
    if exclusive {
        lock.lock()
    } else {
        lock.lock_shared()
    }
    .map_err(|error| EnvError::io(&lock_path, error))?;
    let operation_result = operation();
    let unlock_result = lock
        .unlock()
        .map_err(|error| EnvError::io(&lock_path, error));
    match (operation_result, unlock_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(result), Ok(())) => Ok(result),
    }
}

fn read_unlocked(path: &Path) -> EnvResult<RegistryData> {
    if !path.exists() {
        return Ok(RegistryData::default());
    }
    let bytes = fs::read(path).map_err(|error| EnvError::io(path, error))?;
    serde_json::from_slice(&bytes).map_err(EnvError::serialization)
}

fn write_unlocked(path: &Path, registry: &RegistryData) -> EnvResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| EnvError::invalid("앱 상태 경로가 올바르지 않습니다."))?;
    let mut staged = NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), registry)
        .map_err(EnvError::serialization)?;
    staged
        .write_all(b"\n")
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .persist(path)
        .map_err(|error| EnvError::io(path, error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_preserves_unrelated_registry_state() {
        let app_data = tempfile::tempdir().expect("app data");
        let path = app_data.path().join("projects.json");
        let mut initial = RegistryData {
            last_selected_project_id: Some("existing".to_owned()),
            ..RegistryData::default()
        };
        initial.provider_push_receipts.push(ProviderPushReceipt {
            timestamp_ms: 1,
            project_id: "existing".to_owned(),
            provider: "github".to_owned(),
            source_file: ".env.local".to_owned(),
            destination: "owner/repo".to_owned(),
            succeeded_keys: vec!["DEMO_KEY".to_owned()],
            failed_keys: Vec::new(),
        });
        write(&path, &initial).expect("write registry");

        update(&path, |registry| {
            registry.projects.push(ProjectRegistration {
                id: "new".to_owned(),
                name: "New".to_owned(),
                display_path: "/tmp/new".to_owned(),
                root: PathBuf::from("/tmp/new"),
                file_labels: BTreeMap::new(),
            });
            Ok(())
        })
        .expect("update registry");

        let saved = read(&path).expect("read registry");
        assert_eq!(saved.last_selected_project_id.as_deref(), Some("existing"));
        assert_eq!(saved.provider_push_receipts.len(), 1);
        assert_eq!(saved.projects[0].id, "new");
    }
}

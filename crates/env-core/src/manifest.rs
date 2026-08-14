use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{EnvError, EnvResult};

pub const MANIFEST_FILE_NAME: &str = ".env-manager.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexAccess {
    ReadWrite,
    Protected,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClassificationSource {
    Heuristic,
    User,
    Codex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablePolicy {
    pub codex_access: CodexAccess,
    pub classified_by: ClassificationSource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LinkMember {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkGroup {
    pub id: String,
    pub key: String,
    pub members: Vec<LinkMember>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    #[serde(default)]
    pub ignored_files: Vec<String>,
    #[serde(default)]
    pub ignored_directories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub variables: BTreeMap<String, VariablePolicy>,
    #[serde(default)]
    pub links: Vec<LinkGroup>,
    #[serde(
        default,
        alias = "fileLabels",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub file_labels: BTreeMap<String, String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            scan: ScanConfig::default(),
            variables: BTreeMap::new(),
            links: Vec::new(),
            file_labels: BTreeMap::new(),
        }
    }
}

impl Manifest {
    pub fn access_for(&self, key: &str) -> CodexAccess {
        self.variables
            .get(key)
            .map_or(CodexAccess::Unclassified, |policy| policy.codex_access)
    }

    pub fn linked_count(&self, file: &str, key: &str) -> usize {
        self.link_for(file, key)
            .map_or(0, |link| link.members.len())
    }

    pub fn link_for(&self, file: &str, key: &str) -> Option<&LinkGroup> {
        self.links
            .iter()
            .find(|link| link.key == key && link.members.iter().any(|member| member.file == file))
    }

    pub fn validate(&self) -> EnvResult<()> {
        if self.version != 1 {
            return Err(EnvError::invalid("지원하지 않는 manifest 버전입니다."));
        }

        let mut used_occurrences = BTreeSet::new();
        for link in &self.links {
            if link.members.len() < 2 {
                return Err(EnvError::invalid("연결은 두 개 이상의 멤버가 필요합니다."));
            }
            for member in &link.members {
                validate_relative_path(&member.file)?;
                let occurrence = (member.file.clone(), link.key.clone());
                if !used_occurrences.insert(occurrence) {
                    return Err(EnvError::invalid(
                        "하나의 변수 occurrence가 여러 연결에 포함되어 있습니다.",
                    ));
                }
            }
        }
        for (path, label) in &self.file_labels {
            validate_relative_path(path)?;
            validate_display_name(label)?;
        }
        Ok(())
    }
}

pub fn validate_display_name(name: &str) -> EnvResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 80 || trimmed.chars().any(char::is_control) {
        return Err(EnvError::invalid(
            "표시 이름은 1~80자의 일반 문자여야 합니다.",
        ));
    }
    Ok(())
}

pub struct ManifestStore {
    path: PathBuf,
}

impl ManifestStore {
    pub fn for_root(root: &Path) -> Self {
        Self {
            path: root.join(MANIFEST_FILE_NAME),
        }
    }

    pub fn load(&self) -> EnvResult<Manifest> {
        if !self.path.exists() {
            return Ok(Manifest::default());
        }
        let bytes = fs::read(&self.path).map_err(|error| EnvError::io(&self.path, error))?;
        let manifest =
            serde_json::from_slice::<Manifest>(&bytes).map_err(EnvError::serialization)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn save(&self, manifest: &Manifest) -> EnvResult<()> {
        manifest.validate()?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| EnvError::invalid("manifest 부모 경로가 없습니다."))?;
        let mut staged =
            NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
        serde_json::to_writer_pretty(staged.as_file_mut(), manifest)
            .map_err(EnvError::serialization)?;
        staged
            .write_all(b"\n")
            .map_err(|error| EnvError::io(&self.path, error))?;
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|error| EnvError::io(&self.path, error))?;
        staged
            .persist(&self.path)
            .map_err(|error| EnvError::io(&self.path, error.error))?;
        Ok(())
    }

    pub fn take_legacy_file_labels(&self) -> EnvResult<BTreeMap<String, String>> {
        let mut manifest = self.load()?;
        if manifest.file_labels.is_empty() {
            return Ok(BTreeMap::new());
        }
        let labels = std::mem::take(&mut manifest.file_labels);
        self.save(&manifest)?;
        Ok(labels)
    }
}

fn validate_relative_path(path: &str) -> EnvResult<()> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(EnvError::path_outside(path));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_file_labels_load_but_are_omitted_after_migration() {
        let project = tempfile::tempdir().expect("project");
        let path = project.path().join(MANIFEST_FILE_NAME);
        fs::write(
            &path,
            r#"{"version":1,"fileLabels":{".env.local":"Local env"}}"#,
        )
        .expect("legacy manifest");
        let store = ManifestStore::for_root(project.path());

        let labels = store.take_legacy_file_labels().expect("migrate labels");

        assert_eq!(
            labels.get(".env.local").map(String::as_str),
            Some("Local env")
        );
        let persisted = fs::read_to_string(path).expect("persisted manifest");
        assert!(!persisted.contains("fileLabels"));
        assert!(!persisted.contains("Local env"));
    }
}

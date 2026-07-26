use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub const FAKE_SECRET_CANARY: &str = "fake_test_secret_canary_7f41";

pub struct SyntheticProject {
    root: TempDir,
}

impl SyntheticProject {
    pub fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("synthetic project directory"),
        }
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        assert!(
            content.lines().all(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
                    return true;
                }
                trimmed.split_once('=').is_some_and(|(_, value)| {
                    let value = value.trim().trim_matches(['\'', '"']);
                    value.is_empty() || value.starts_with("fake_")
                })
            }),
            "fixture values must be visibly fake"
        );
        let path = self.root.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(&path, content).expect("fixture write");
        path
    }

    pub fn read(&self, relative: &str) -> Vec<u8> {
        fs::read(self.root.path().join(relative)).expect("fixture read")
    }
}

impl Default for SyntheticProject {
    fn default() -> Self {
        Self::new()
    }
}

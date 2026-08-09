use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{EnvError, EnvResult, FileRevision};

const GITIGNORE_PATH: &str = ".gitignore";
const GUARD_COMMENT: &str = "# Env Manager: local environment values";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitSafetyState {
    Protected,
    NeedsAttention,
    NotRepository,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSafetyProjection {
    pub state: GitSafetyState,
    pub ignored_files: Vec<String>,
    pub missing_ignore_files: Vec<String>,
    pub tracked_files: Vec<String>,
}

impl GitSafetyProjection {
    pub fn not_repository() -> Self {
        Self {
            state: GitSafetyState::NotRepository,
            ignored_files: Vec::new(),
            missing_ignore_files: Vec::new(),
            tracked_files: Vec::new(),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            state: GitSafetyState::Unavailable,
            ignored_files: Vec::new(),
            missing_ignore_files: Vec::new(),
            tracked_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitignoreUpdateSummary {
    pub added_patterns: Vec<String>,
    pub tracked_files: Vec<String>,
}

pub fn inspect_git_safety(root: &Path, managed_files: &[PathBuf]) -> GitSafetyProjection {
    match GitInspector::open(root) {
        GitOpenResult::Ready(inspector) => inspector.inspect(managed_files),
        GitOpenResult::NotRepository => GitSafetyProjection::not_repository(),
        GitOpenResult::Unavailable => GitSafetyProjection::unavailable(),
    }
}

pub fn apply_gitignore_guard(
    root: &Path,
    managed_files: &[PathBuf],
) -> EnvResult<GitignoreUpdateSummary> {
    let inspector = match GitInspector::open(root) {
        GitOpenResult::Ready(inspector) => inspector,
        GitOpenResult::NotRepository => {
            return Err(EnvError::invalid("Git 저장소가 아닌 프로젝트입니다."));
        }
        GitOpenResult::Unavailable => {
            return Err(EnvError::invalid("Git ignore 상태를 확인할 수 없습니다."));
        }
    };
    let projection = inspector.inspect(managed_files);
    if projection.state == GitSafetyState::Unavailable {
        return Err(EnvError::invalid("Git ignore 상태를 확인할 수 없습니다."));
    }

    let patterns = projection
        .missing_ignore_files
        .iter()
        .map(|path| exact_gitignore_pattern(path))
        .collect::<Vec<_>>();
    if !patterns.is_empty() {
        append_gitignore_patterns(root, &patterns)?;
    }

    Ok(GitignoreUpdateSummary {
        added_patterns: patterns,
        tracked_files: projection.tracked_files,
    })
}

enum GitOpenResult {
    Ready(GitInspector),
    NotRepository,
    Unavailable,
}

struct GitInspector {
    root: PathBuf,
    tracked_files: BTreeSet<String>,
}

impl GitInspector {
    fn open(root: &Path) -> GitOpenResult {
        let repository = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match repository {
            Err(_) => GitOpenResult::Unavailable,
            Ok(status) if !status.success() => GitOpenResult::NotRepository,
            Ok(_) => {
                let tracked = Command::new("git")
                    .args(["ls-files", "--cached", "-z", "--", "."])
                    .current_dir(root)
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .output();
                match tracked {
                    Ok(output) if output.status.success() => {
                        let tracked_files = output
                            .stdout
                            .split(|byte| *byte == 0)
                            .filter(|path| !path.is_empty())
                            .map(|path| String::from_utf8_lossy(path).into_owned())
                            .collect();
                        GitOpenResult::Ready(Self {
                            root: root.to_path_buf(),
                            tracked_files,
                        })
                    }
                    _ => GitOpenResult::Unavailable,
                }
            }
        }
    }

    fn inspect(&self, managed_files: &[PathBuf]) -> GitSafetyProjection {
        let mut ignored_files = Vec::new();
        let mut missing_ignore_files = Vec::new();
        let mut tracked_files = Vec::new();
        let managed_paths = managed_files
            .iter()
            .map(|relative| manifest_path(relative))
            .collect::<Vec<_>>();
        let Some(ignored) = self.ignored_paths(&managed_paths) else {
            return GitSafetyProjection::unavailable();
        };

        for path in managed_paths {
            if self.tracked_files.contains(&path) {
                tracked_files.push(path.clone());
            }
            if ignored.contains(&path) {
                ignored_files.push(path);
            } else {
                missing_ignore_files.push(path);
            }
        }

        let state = if missing_ignore_files.is_empty() && tracked_files.is_empty() {
            GitSafetyState::Protected
        } else {
            GitSafetyState::NeedsAttention
        };
        GitSafetyProjection {
            state,
            ignored_files,
            missing_ignore_files,
            tracked_files,
        }
    }

    fn ignored_paths(&self, paths: &[String]) -> Option<BTreeSet<String>> {
        if paths.is_empty() {
            return Some(BTreeSet::new());
        }
        let mut child = Command::new("git")
            .args(["check-ignore", "--no-index", "-z", "--stdin"])
            .current_dir(&self.root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        {
            let stdin = child.stdin.as_mut()?;
            for path in paths {
                stdin.write_all(path.as_bytes()).ok()?;
                stdin.write_all(&[0]).ok()?;
            }
        }
        let output = child.wait_with_output().ok()?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return None;
        }
        Some(
            output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
                .map(|path| String::from_utf8_lossy(path).into_owned())
                .collect(),
        )
    }
}

fn append_gitignore_patterns(root: &Path, patterns: &[String]) -> EnvResult<()> {
    let path = root.join(GITIGNORE_PATH);
    let existing = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(EnvError::invalid(".gitignore가 일반 파일이 아닙니다."));
            }
            fs::read(&path).map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(EnvError::io(Path::new(GITIGNORE_PATH), error)),
    };

    let mut proposed = existing.clone();
    if !proposed.is_empty() && !proposed.ends_with(b"\n") {
        proposed.push(b'\n');
    }
    if !proposed.is_empty() && !proposed.ends_with(b"\n\n") {
        proposed.push(b'\n');
    }
    proposed.extend_from_slice(GUARD_COMMENT.as_bytes());
    proposed.push(b'\n');
    for pattern in patterns {
        proposed.extend_from_slice(pattern.as_bytes());
        proposed.push(b'\n');
    }

    if existing.is_empty() && !path.exists() {
        let mut created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?;
        created
            .write_all(&proposed)
            .and_then(|()| created.sync_all())
            .map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?;
        return Ok(());
    }

    let expected_revision = FileRevision::from_bytes(&existing);
    let permissions = fs::metadata(&path)
        .map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?
        .permissions();
    let mut staged = NamedTempFile::new_in(root).map_err(|error| EnvError::io(root, error))?;
    staged
        .write_all(&proposed)
        .and_then(|()| staged.as_file_mut().set_permissions(permissions))
        .and_then(|()| staged.as_file_mut().sync_all())
        .map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?;
    let current =
        fs::read(&path).map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error))?;
    if FileRevision::from_bytes(&current) != expected_revision {
        return Err(EnvError::changed_externally(Path::new(GITIGNORE_PATH)));
    }
    staged
        .persist(&path)
        .map_err(|error| EnvError::io(Path::new(GITIGNORE_PATH), error.error))?;
    Ok(())
}

fn exact_gitignore_pattern(path: &str) -> String {
    let escaped = path.chars().fold(String::new(), |mut output, character| {
        if matches!(character, '\\' | '!' | '#' | '[' | ']' | '*' | '?' | ' ') {
            output.push('\\');
        }
        output.push(character);
        output
    });
    format!("/{escaped}")
}

fn manifest_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use env_test_support::SyntheticProject;

    use super::*;

    fn init_git(project: &SyntheticProject) {
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(project.root())
            .status()
            .expect("git must be available in tests");
        assert!(status.success());
    }

    #[test]
    fn reports_ignored_missing_and_tracked_files_without_reading_values() {
        let project = SyntheticProject::new();
        project.write(".env", "TOKEN=fake_canary_ignored\n");
        project.write("apps/web/.env.local", "TOKEN=fake_canary_missing\n");
        project.write("apps/api/.env.dev", "TOKEN=fake_canary_tracked\n");
        project.write(".gitignore", "/.env\n");
        init_git(&project);
        let status = Command::new("git")
            .args(["add", "-f", "--", "apps/api/.env.dev"])
            .current_dir(project.root())
            .status()
            .expect("git add");
        assert!(status.success());

        let projection = inspect_git_safety(
            project.root(),
            &[
                PathBuf::from(".env"),
                PathBuf::from("apps/web/.env.local"),
                PathBuf::from("apps/api/.env.dev"),
            ],
        );

        assert_eq!(projection.state, GitSafetyState::NeedsAttention);
        assert_eq!(projection.ignored_files, vec![".env"]);
        assert_eq!(
            projection.missing_ignore_files,
            vec!["apps/web/.env.local", "apps/api/.env.dev"]
        );
        assert_eq!(projection.tracked_files, vec!["apps/api/.env.dev"]);
    }

    #[test]
    fn adds_exact_rules_without_changing_the_git_index() {
        let project = SyntheticProject::new();
        project.write("apps/web/.env.local", "TOKEN=fake_canary_missing\n");
        init_git(&project);

        let summary =
            apply_gitignore_guard(project.root(), &[PathBuf::from("apps/web/.env.local")])
                .expect("apply guard");

        assert_eq!(summary.added_patterns, vec!["/apps/web/.env.local"]);
        assert!(summary.tracked_files.is_empty());
        let gitignore = fs::read_to_string(project.root().join(".gitignore"))
            .expect("read synthetic gitignore");
        assert_eq!(
            gitignore,
            "# Env Manager: local environment values\n/apps/web/.env.local\n"
        );
        let projection =
            inspect_git_safety(project.root(), &[PathBuf::from("apps/web/.env.local")]);
        assert_eq!(projection.state, GitSafetyState::Protected);
        let tracked = Command::new("git")
            .args(["ls-files", "--error-unmatch", "--", "apps/web/.env.local"])
            .current_dir(project.root())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("check Git index");
        assert!(!tracked.success(), "guard must not stage the env file");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_existing_gitignore_content_and_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let project = SyntheticProject::new();
        project.write(".env.local", "TOKEN=fake_canary_missing\n");
        let gitignore_path = project.write(".gitignore", "*.log\n");
        fs::set_permissions(&gitignore_path, fs::Permissions::from_mode(0o640))
            .expect("set synthetic permissions");
        init_git(&project);

        apply_gitignore_guard(project.root(), &[PathBuf::from(".env.local")]).expect("apply guard");

        let gitignore = fs::read_to_string(&gitignore_path).expect("read synthetic gitignore");
        assert_eq!(
            gitignore,
            "*.log\n\n# Env Manager: local environment values\n/.env.local\n"
        );
        assert_eq!(
            fs::metadata(&gitignore_path).expect("metadata").mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn escapes_gitignore_metacharacters() {
        assert_eq!(
            exact_gitignore_pattern("apps/my app/[local]/.env.dev"),
            "/apps/my\\ app/\\[local\\]/.env.dev"
        );
    }
}

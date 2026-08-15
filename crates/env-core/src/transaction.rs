use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::{Document, EnvError, EnvResult};

#[derive(Clone, PartialEq, Eq)]
pub struct FileRevision([u8; 32]);

impl FileRevision {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }
}

pub struct PlannedFileChange {
    pub relative_path: PathBuf,
    pub expected_revision: FileRevision,
    pub proposed_bytes: Vec<u8>,
}

pub struct TransactionPlan {
    pub files: Vec<PlannedFileChange>,
}

impl TransactionPlan {
    pub fn new(files: Vec<PlannedFileChange>) -> Self {
        Self { files }
    }

    pub fn commit(self, root: &Path) -> EnvResult<()> {
        let root = root
            .canonicalize()
            .map_err(|error| EnvError::io(root, error))?;
        let mut prepared = Vec::with_capacity(self.files.len());

        for change in self.files {
            let target = safe_target(&root, &change.relative_path)?;
            let original =
                fs::read(&target).map_err(|error| EnvError::io(&change.relative_path, error))?;
            if FileRevision::from_bytes(&original) != change.expected_revision {
                return Err(EnvError::changed_externally(&change.relative_path));
            }
            Document::parse(change.proposed_bytes.clone(), &change.relative_path)?;
            let permissions = fs::metadata(&target)
                .map_err(|error| EnvError::io(&change.relative_path, error))?
                .permissions();
            let parent = target
                .parent()
                .ok_or_else(|| EnvError::invalid("파일 부모 경로가 없습니다."))?;
            let mut staged =
                NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
            staged
                .write_all(&change.proposed_bytes)
                .map_err(|error| EnvError::io(&change.relative_path, error))?;
            staged
                .as_file_mut()
                .set_permissions(permissions)
                .map_err(|error| EnvError::io(&change.relative_path, error))?;
            staged
                .as_file_mut()
                .sync_all()
                .map_err(|error| EnvError::io(&change.relative_path, error))?;
            prepared.push(PreparedChange {
                target,
                relative_path: change.relative_path,
                original,
                staged,
            });
        }

        let mut committed = Vec::<CommittedChange>::new();
        for prepared_change in prepared {
            let PreparedChange {
                target,
                relative_path,
                original,
                staged,
            } = prepared_change;
            match staged.persist(&target) {
                Ok(_) => committed.push(CommittedChange {
                    target,
                    relative_path,
                    original,
                }),
                Err(error) => {
                    let commit_error = EnvError::io(&relative_path, error.error);
                    let rollback_result = rollback(committed);
                    return match rollback_result {
                        Ok(()) => Err(commit_error),
                        Err(rollback_error) => Err(EnvError::transaction(format!(
                            "파일 저장과 복구에 실패했습니다: {rollback_error}"
                        ))),
                    };
                }
            }
        }

        Ok(())
    }
}

struct PreparedChange {
    target: PathBuf,
    relative_path: PathBuf,
    original: Vec<u8>,
    staged: NamedTempFile,
}

struct CommittedChange {
    target: PathBuf,
    relative_path: PathBuf,
    original: Vec<u8>,
}

fn rollback(committed: Vec<CommittedChange>) -> EnvResult<()> {
    for change in committed.into_iter().rev() {
        let parent = change
            .target
            .parent()
            .ok_or_else(|| EnvError::invalid("복구할 파일의 부모 경로가 없습니다."))?;
        let mut staged =
            NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
        staged
            .write_all(&change.original)
            .map_err(|error| EnvError::io(&change.relative_path, error))?;
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|error| EnvError::io(&change.relative_path, error))?;
        staged
            .persist(&change.target)
            .map_err(|error| EnvError::io(&change.relative_path, error.error))?;
    }
    Ok(())
}

pub(crate) fn safe_target(root: &Path, relative: &Path) -> EnvResult<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(EnvError::path_outside(relative));
    }
    let target = root.join(relative);
    let canonical = target
        .canonicalize()
        .map_err(|error| EnvError::io(relative, error))?;
    if !canonical.starts_with(root) {
        return Err(EnvError::path_outside(relative));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use env_test_support::SyntheticProject;

    use super::*;

    #[test]
    fn rejects_stale_revision() {
        let project = SyntheticProject::new();
        let path = project.write(".env", "PORT=fake_3000\n");
        let original = fs::read(&path).expect("read");
        let revision = FileRevision::from_bytes(&original);
        fs::write(&path, "PORT=fake_external\n").expect("external write");

        let result = TransactionPlan::new(vec![PlannedFileChange {
            relative_path: PathBuf::from(".env"),
            expected_revision: revision,
            proposed_bytes: b"PORT=fake_4000\n".to_vec(),
        }])
        .commit(project.root());

        assert_eq!(
            result.expect_err("must reject").code(),
            crate::EnvErrorCode::FileChangedExternally
        );
    }

    #[test]
    fn replaces_an_existing_file_through_the_staged_commit_path() {
        let project = SyntheticProject::new();
        let path = project.write(".env.local", "PORT=fake_3000\r\n");
        let original = fs::read(&path).expect("read original");

        TransactionPlan::new(vec![PlannedFileChange {
            relative_path: PathBuf::from(".env.local"),
            expected_revision: FileRevision::from_bytes(&original),
            proposed_bytes: b"PORT=fake_4000\r\n".to_vec(),
        }])
        .commit(project.root())
        .expect("replace existing file");

        assert_eq!(
            fs::read(path).expect("read committed file"),
            b"PORT=fake_4000\r\n"
        );
    }
}

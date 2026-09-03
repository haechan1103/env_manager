use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

use crate::discovery::{is_env_candidate, to_manifest_path};
use crate::{
    DiscoveryOptions, Document, EnvError, EnvErrorCode, EnvResult, FileRevision, Manifest,
};

const MAX_ENCRYPTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECRYPTED_ZIP_BYTES: u64 = 40 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TeamImportOccurrenceState {
    New,
    Unchanged,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportOccurrenceProjection {
    pub id: String,
    pub key: String,
    pub state: TeamImportOccurrenceState,
    pub link_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportFileProjection {
    pub path: String,
    pub target_path: String,
    pub occurrences: Vec<TeamImportOccurrenceProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportPreview {
    pub files: Vec<TeamImportFileProjection>,
    pub new_count: usize,
    pub unchanged_count: usize,
    pub conflict_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImportSummary {
    pub added_count: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub kept_local_count: usize,
    pub affected_files: Vec<String>,
}

pub struct TeamImportPlan {
    root: PathBuf,
    manifest: Manifest,
    entries: Vec<TeamImportEntry>,
    preview: TeamImportPreview,
}

struct TeamImportEntry {
    source_path: PathBuf,
    target_path: PathBuf,
    package_bytes: Zeroizing<Vec<u8>>,
    occurrences: Vec<IncomingOccurrence>,
    expected: ExpectedTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TeamImportValueSide {
    Local,
    Shared,
}

struct IncomingOccurrence {
    id: String,
    key: String,
    decoded_value: Zeroizing<String>,
    raw_value: Zeroizing<Vec<u8>>,
    state: TeamImportOccurrenceState,
    link_id: Option<String>,
}

enum ExpectedTarget {
    Missing,
    Existing(FileRevision),
}

mod apply;
mod package;
mod plan;
mod preview;

use apply::{apply_entries, build_changes, validate_link_invariants, verify_expected};
pub use package::plan_encrypted_team_import;
use package::{build_entry, package_invalid, validate_package_path};
use preview::{
    expand_linked_conflicts, is_managed_import_path, occurrence_id, project_preview,
    validate_package_links,
};

#[cfg(test)]
mod tests {
    use age::secrecy::SecretString;
    use env_test_support::SyntheticProject;

    use super::*;
    use crate::{ExportOccurrence, LinkGroup, LinkMember, export_project_env};

    fn encrypted_fixture(
        selection: Option<&[ExportOccurrence]>,
    ) -> (SyntheticProject, PathBuf, SecretString) {
        let source = SyntheticProject::new();
        source.write(
            ".env.local",
            "# sender comment\nTOKEN=fake_team_import_canary\nPORT=fake_3000\n",
        );
        source.write("apps/web/.env.dev", "PORT=fake_4000\n");
        source.write(".dev.vars", "WORKER_MODE=fake_local\n");
        let package = source.root().join("team-env.zip.age");
        let passphrase = SecretString::from("fake-team-passphrase-2026".to_owned());
        export_project_env(
            source.root(),
            &Manifest::default(),
            &package,
            Some(passphrase.clone()),
            selection,
        )
        .expect("package");
        (source, package, passphrase)
    }

    #[test]
    fn applies_new_files_without_exposing_values() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("import plan");
        let preview_json = serde_json::to_string(plan.preview()).expect("preview json");

        assert_eq!(plan.preview().new_count, 4);
        assert!(!preview_json.contains("fake_team_import_canary"));
        let summary = plan.apply(&[]).expect("apply");
        assert_eq!(summary.added_count, 4);
        assert_eq!(destination.read(".dev.vars"), b"WORKER_MODE=fake_local\n");
        assert!(
            destination
                .read(".env.local")
                .starts_with(b"# sender comment")
        );
    }

    #[test]
    fn partial_share_merges_only_selected_values_and_preserves_local_content() {
        let selection = vec![ExportOccurrence {
            file: ".env.local".to_owned(),
            key: "TOKEN".to_owned(),
        }];
        let (_source, package, passphrase) = encrypted_fixture(Some(&selection));
        let destination = SyntheticProject::new();
        destination.write(
            ".env.local",
            "# local comment\nTOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\n",
        );

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let conflict_id = plan.preview().files[0].occurrences[0].id.clone();
        let summary = plan.apply(&[conflict_id]).expect("apply shared conflict");

        assert_eq!(summary.updated_count, 1);
        assert_eq!(
            destination.read(".env.local"),
            b"# local comment\nTOKEN=fake_team_import_canary\nLOCAL_ONLY=fake_keep_me\n"
        );
        assert!(!destination.root().join("apps/web/.env.dev").exists());
    }

    #[test]
    fn remaps_one_incoming_file_and_reveals_only_an_explicit_conflict_side() {
        let selection = vec![ExportOccurrence {
            file: ".env.local".to_owned(),
            key: "TOKEN".to_owned(),
        }];
        let (_source, package, passphrase) = encrypted_fixture(Some(&selection));
        let destination = SyntheticProject::new();
        destination.write(
            ".env.staging",
            "TOKEN=fake_staging_local\nLOCAL_ONLY=fake_keep_me\n",
        );

        let mut plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let preview = plan
            .remap_file(".env.local", ".env.staging")
            .expect("remap");
        assert_eq!(preview.files[0].path, ".env.local");
        assert_eq!(preview.files[0].target_path, ".env.staging");
        assert_eq!(preview.conflict_count, 1);

        let conflict_id = preview.files[0].occurrences[0].id.clone();
        let local = plan
            .reveal_conflict(&conflict_id, TeamImportValueSide::Local)
            .expect("local reveal");
        let shared = plan
            .reveal_conflict(&conflict_id, TeamImportValueSide::Shared)
            .expect("shared reveal");
        assert_eq!(local.as_str(), "fake_staging_local");
        assert_eq!(shared.as_str(), "fake_team_import_canary");

        plan.apply(&[conflict_id]).expect("apply remapped value");
        assert_eq!(
            destination.read(".env.staging"),
            b"TOKEN=fake_team_import_canary\nLOCAL_ONLY=fake_keep_me\n"
        );
        assert!(!destination.root().join(".env.local").exists());
    }

    #[test]
    fn rejects_mapping_two_package_files_to_one_target() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");
        let mut plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");

        let result = plan.remap_file(".env.local", "apps/web/.env.dev");
        assert!(matches!(result, Err(error) if error.code() == EnvErrorCode::PackageInvalid));
    }

    #[test]
    fn keeping_a_conflict_preserves_the_local_value_while_adding_new_variables() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        destination.write(
            ".env.local",
            "TOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\n",
        );
        fs::create_dir_all(destination.root().join("apps/web")).expect("destination directory");

        let plan = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            passphrase,
        )
        .expect("plan");
        let summary = plan.apply(&[]).expect("keep local conflict");

        assert_eq!(summary.kept_local_count, 1);
        assert_eq!(
            destination.read(".env.local"),
            b"TOKEN=fake_local_existing\nLOCAL_ONLY=fake_keep_me\nPORT=fake_3000\n"
        );
    }

    #[test]
    fn wrong_passphrase_returns_a_value_free_error() {
        let (_source, package, _passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        let result = plan_encrypted_team_import(
            destination.root(),
            &Manifest::default(),
            &package,
            SecretString::from("fake-wrong-passphrase-2026".to_owned()),
        );
        let error = match result {
            Ok(_) => panic!("wrong passphrase must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code(), EnvErrorCode::PackageDecryptFailed);
        assert!(!error.to_string().contains("fake_team_import_canary"));
    }

    #[test]
    fn rejects_files_excluded_by_the_shared_manifest() {
        let (_source, package, passphrase) = encrypted_fixture(None);
        let destination = SyntheticProject::new();
        let mut manifest = Manifest::default();
        manifest.scan.ignored_files.push(".env.local".to_owned());
        let result =
            plan_encrypted_team_import(destination.root(), &manifest, &package, passphrase);
        assert!(matches!(result, Err(error) if error.code() == EnvErrorCode::PackageInvalid));
    }

    #[test]
    fn linked_conflicts_are_applied_as_one_group() {
        let source = SyntheticProject::new();
        source.write(".env.local", "TOKEN=fake_shared\n");
        source.write(".env.staging", "TOKEN=fake_shared\n");
        let package = source.root().join("linked.zip.age");
        let passphrase = SecretString::from("fake-linked-passphrase-2026".to_owned());
        export_project_env(
            source.root(),
            &Manifest::default(),
            &package,
            Some(passphrase.clone()),
            None,
        )
        .expect("package");
        let destination = SyntheticProject::new();
        destination.write(".env.local", "TOKEN=fake_old\n");
        destination.write(".env.staging", "TOKEN=fake_old\n");
        let mut manifest = Manifest::default();
        manifest.links.push(LinkGroup {
            id: "token-link".to_owned(),
            key: "TOKEN".to_owned(),
            members: vec![
                LinkMember {
                    file: ".env.local".to_owned(),
                },
                LinkMember {
                    file: ".env.staging".to_owned(),
                },
            ],
        });

        let plan = plan_encrypted_team_import(destination.root(), &manifest, &package, passphrase)
            .expect("plan");
        let selected = plan.preview().files[0].occurrences[0].id.clone();
        let summary = plan.apply(&[selected]).expect("linked apply");
        assert_eq!(summary.updated_count, 2);
        assert_eq!(destination.read(".env.local"), b"TOKEN=fake_shared\n");
        assert_eq!(destination.read(".env.staging"), b"TOKEN=fake_shared\n");
    }

    #[test]
    fn rejects_traversal_and_non_env_entries() {
        assert!(validate_package_path("workers/api/.dev.vars.production").is_ok());
        for name in [
            "../.env",
            "/.env",
            ".env.example",
            "notes.txt",
            "apps\\web\\.env",
        ] {
            assert!(validate_package_path(name).is_err(), "must reject {name}");
        }
    }
}

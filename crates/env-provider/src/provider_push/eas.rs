use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use env_core::ProviderValue;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use crate::provider_adapter::{self, AdapterStrategy, ResolvedAdapter};

use super::cli::provider_command;
use super::error::{ProviderPushError, invalid_request, invalid_target};
use super::model::{
    EXPO_EAS_ID, EasAccessContext, EasTargetContext, ProviderEntryKind, ProviderPushRequest,
    ProviderPushResult,
};
use super::validation::{source_directory, validate_simple_target};

const PROMPT: &[u8] = b"Variable value:";
#[cfg(windows)]
const CURSOR_POSITION_REQUEST: &[u8] = b"\x1b[6n";
#[cfg(windows)]
const CURSOR_POSITION_RESPONSE: &[u8] = b"\x1b[1;1R";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_PREFLIGHT_OUTPUT: usize = 64 * 1024;
const MAX_PROMPT_WINDOW: usize = 4096;

pub(super) struct PreparedEasProvider {
    root: PathBuf,
    adapter: ResolvedAdapter,
}

mod access;
mod discovery;
mod transport;
mod workflow;

pub use access::inspect_access;
#[cfg(test)]
use access::parse_project_info;
use access::{configured_project_matches, inspect_project, project_matches, resolve_adapter};
pub use discovery::detect_target;
use transport::{execute_hidden_prompt, set_args};
pub(super) use workflow::{prepare, push};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_eas_target_without_reading_env_values() {
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(project.path().join("apps/mobile")).expect("mobile");
        std::fs::write(
            project.path().join("apps/mobile/.env"),
            "SYNTHETIC_KEY=canary\n",
        )
        .expect("env");
        std::fs::write(project.path().join("apps/mobile/eas.json"), r#"{"build":{"dev":{"environment":"development"},"prod":{"environment":"production"}}}"#).expect("eas");
        std::fs::write(
            project.path().join("apps/mobile/app.json"),
            r#"{"expo":{"slug":"travel-pieces","extra":{"eas":{"projectId":"synthetic-id"}}}}"#,
        )
        .expect("app");

        let target = detect_target(project.path(), "apps/mobile/.env").expect("target");
        assert_eq!(target.project.as_deref(), Some("travel-pieces"));
        assert_eq!(target.project_id.as_deref(), Some("synthetic-id"));
        assert_eq!(target.environments, vec!["development", "production"]);
    }

    #[test]
    fn eas_arguments_never_contain_the_value_or_value_flag() {
        let canary = "synthetic-eas-secret-canary";
        let args = set_args(
            "EXPO_PUBLIC_KAKAO_NATIVE_APP_KEY",
            ProviderEntryKind::Sensitive,
            &[
                "development".to_owned(),
                "preview".to_owned(),
                "production".to_owned(),
            ],
        )
        .expect("args");
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!rendered.contains(canary));
        assert!(!rendered.contains("--value"));
        assert!(rendered.contains("--visibility sensitive"));
    }

    #[test]
    fn expo_public_cannot_be_misclassified_as_eas_secret() {
        let error = set_args(
            "EXPO_PUBLIC_KAKAO_NATIVE_APP_KEY",
            ProviderEntryKind::Secret,
            &["production".to_owned()],
        )
        .expect_err("reject secret");
        assert_eq!(error.code, "EAS_PUBLIC_SECRET_UNSUPPORTED");
    }

    #[test]
    fn parses_and_matches_project_info() {
        let info = parse_project_info(
            b"fullName  @haechan/travel-pieces\nID  2bb051f4-155a-4978-a1b5-934596bd8f3a\n",
        )
        .expect("info");
        assert!(project_matches("travel-pieces", &info));
        assert!(project_matches("@haechan/travel-pieces", &info));
        assert!(!project_matches("other-project", &info));
        assert!(configured_project_matches(
            &EasTargetContext {
                project: Some("travel-pieces".to_owned()),
                project_id: Some("2bb051f4-155a-4978-a1b5-934596bd8f3a".to_owned()),
                environments: Vec::new(),
                config_path: Some("apps/mobile/eas.json".to_owned()),
            },
            &info,
        ));
        assert!(!configured_project_matches(
            &EasTargetContext {
                project: Some("travel-pieces".to_owned()),
                project_id: Some("different-project-id".to_owned()),
                environments: Vec::new(),
                config_path: Some("apps/mobile/eas.json".to_owned()),
            },
            &info,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn pty_transport_waits_for_prompt_and_keeps_canary_out_of_arguments() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("directory");
        let executable = directory.path().join("fake-eas");
        std::fs::write(&executable, "#!/bin/sh\nprintf 'Variable value:'\nIFS= read -r value\n[ \"$value\" = \"synthetic-eas-pty-canary\" ]\n").expect("script");
        let mut permissions = std::fs::metadata(&executable)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&executable, permissions).expect("permissions");
        let args = set_args(
            "SYNTHETIC_KEY",
            ProviderEntryKind::Sensitive,
            &["development".to_owned()],
        )
        .expect("args");

        assert!(execute_hidden_prompt(
            &executable,
            directory.path(),
            &args,
            "synthetic-eas-pty-canary"
        ));
        assert!(
            args.iter()
                .all(|arg| !arg.to_string_lossy().contains("synthetic-eas-pty-canary"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_conpty_transport_waits_for_hidden_prompt() {
        let directory = tempfile::tempdir().expect("directory");
        let executable = directory.path().join("fake-eas.cmd");
        std::fs::write(
            &executable,
            "@echo off\r\npowershell.exe -NoLogo -NoProfile -NonInteractive -Command \"[Console]::Out.Write('Variable value:'); [Console]::Out.Flush(); $value = [Console]::In.ReadLine(); if ($value -eq 'synthetic-eas-pty-canary') { exit 0 } else { exit 1 }\"\r\nexit /b %errorlevel%\r\n",
        )
        .expect("script");
        let args = set_args(
            "SYNTHETIC_KEY",
            ProviderEntryKind::Sensitive,
            &["development".to_owned()],
        )
        .expect("args");

        assert!(execute_hidden_prompt(
            &executable,
            directory.path(),
            &args,
            "synthetic-eas-pty-canary"
        ));
        assert!(
            args.iter()
                .all(|arg| !arg.to_string_lossy().contains("synthetic-eas-pty-canary"))
        );
    }
}

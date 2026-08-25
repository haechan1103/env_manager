use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use directories::BaseDirs;

pub(super) fn run_with_stdin(
    executable: &Path,
    root: &Path,
    args: &[OsString],
    stdin: &[u8],
) -> bool {
    let mut command = provider_command(executable, args);
    let mut child = match command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut pipe| pipe.write_all(stdin).is_ok());
    wrote && child.wait().is_ok_and(|status| status.success())
}

pub(crate) fn provider_command(executable: &Path, args: &[OsString]) -> Command {
    let mut command = if cfg!(windows)
        && executable.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        }) {
        let mut command = Command::new("cmd.exe");
        command.arg("/D").arg("/S").arg("/C").arg(executable);
        command.args(args);
        command
    } else {
        let mut command = Command::new(executable);
        command.args(args);
        command
    };
    suppress_console_window(&mut command);
    command
}

pub(super) fn background_command(executable: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(executable);
    suppress_console_window(&mut command);
    command
}

#[cfg(windows)]
fn suppress_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn suppress_console_window(_command: &mut Command) {}

pub(crate) fn find_cli(name: &str, root: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if matches!(name, "wrangler" | "eas") {
        candidates.push(root.join("node_modules/.bin").join(if cfg!(windows) {
            format!("{name}.cmd")
        } else {
            name.to_owned()
        }));
    }
    let executable_names = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
        ]
    } else {
        vec![name.to_owned()]
    };
    candidates.extend(
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .flat_map(|directory| {
                executable_names
                    .iter()
                    .map(move |executable| directory.join(executable))
            }),
    );
    if let Some(base) = BaseDirs::new() {
        for directory in [
            base.home_dir().join(".local/bin"),
            base.home_dir().join(".cargo/bin"),
        ] {
            for executable in &executable_names {
                candidates.push(directory.join(executable));
            }
        }
        if matches!(name, "wrangler" | "eas") && cfg!(windows) {
            candidates.push(
                base.home_dir()
                    .join(format!("AppData/Roaming/npm/{name}.cmd")),
            );
        }
    }
    if !cfg!(windows) {
        for executable in &executable_names {
            candidates.push(PathBuf::from("/opt/homebrew/bin").join(executable));
            candidates.push(PathBuf::from("/usr/local/bin").join(executable));
            candidates.push(PathBuf::from("/usr/bin").join(executable));
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_cmd_provider_launcher_receives_standard_input() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let launcher = directory.path().join("fake-provider.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nset /p payload=\r\nif \"%payload%\"==\"fake-provider-input\" exit /b 0\r\nexit /b 1\r\n",
        )
        .expect("write launcher");

        assert!(run_with_stdin(
            &launcher,
            directory.path(),
            &[],
            b"fake-provider-input\n",
        ));
    }
}

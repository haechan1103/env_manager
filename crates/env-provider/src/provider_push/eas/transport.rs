use super::*;

pub(super) fn set_args(
    key: &str,
    kind: ProviderEntryKind,
    environments: &[String],
) -> Result<Vec<OsString>, ProviderPushError> {
    validate_simple_target(key)?;
    let visibility = match kind {
        ProviderEntryKind::Plaintext => "plaintext",
        ProviderEntryKind::Sensitive => "sensitive",
        ProviderEntryKind::Secret if !key.starts_with("EXPO_PUBLIC_") => "secret",
        ProviderEntryKind::Secret => {
            return Err(ProviderPushError {
                code: "EAS_PUBLIC_SECRET_UNSUPPORTED",
                message: "EXPO_PUBLIC_ 변수는 앱 번들에 필요한 공개 식별자이므로 EAS Secret으로 전송할 수 없습니다.",
            });
        }
        ProviderEntryKind::Variable => {
            return Err(invalid_request(
                "Expo EAS에서 지원하지 않는 변수 유형입니다.",
            ));
        }
    };
    let mut args = vec![
        OsString::from("env:set"),
        OsString::from("--name"),
        OsString::from(key),
        OsString::from("--type"),
        OsString::from("string"),
        OsString::from("--visibility"),
        OsString::from(visibility),
        OsString::from("--scope"),
        OsString::from("project"),
    ];
    for environment in environments {
        args.push(OsString::from("--environment"));
        args.push(OsString::from(environment));
    }
    Ok(args)
}

pub(super) fn execute_hidden_prompt(
    executable: &Path,
    root: &Path,
    args: &[OsString],
    value: &str,
) -> bool {
    #[derive(Clone, Copy)]
    enum PtyEvent {
        Prompt,
        #[cfg(windows)]
        CursorPositionRequest,
    }

    let system = native_pty_system();
    let pair = match system.openpty(PtySize {
        rows: 24,
        cols: 100,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(_) => return false,
    };
    let mut command = pty_command(executable, args);
    command.cwd(root);
    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(_) => return false,
    };
    drop(pair.slave);
    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => {
            let _ = child.kill();
            return false;
        }
    };
    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => {
            let _ = child.kill();
            return false;
        }
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader_thread = thread::spawn(move || {
        let mut chunk = Zeroizing::new([0_u8; 512]);
        let mut window = Zeroizing::new(Vec::<u8>::with_capacity(MAX_PROMPT_WINDOW));
        let mut prompt_reported = false;
        #[cfg(windows)]
        let mut cursor_reported = false;
        while let Ok(count) = reader.read(&mut *chunk) {
            if count == 0 {
                break;
            }
            if !prompt_reported {
                window.extend_from_slice(&chunk[..count]);
                #[cfg(windows)]
                if !cursor_reported
                    && window
                        .windows(CURSOR_POSITION_REQUEST.len())
                        .any(|candidate| candidate == CURSOR_POSITION_REQUEST)
                {
                    if sender.send(PtyEvent::CursorPositionRequest).is_err() {
                        break;
                    }
                    cursor_reported = true;
                }
                if window
                    .windows(PROMPT.len())
                    .any(|candidate| candidate == PROMPT)
                {
                    let _ = sender.send(PtyEvent::Prompt);
                    prompt_reported = true;
                    window.zeroize();
                } else if window.len() > MAX_PROMPT_WINDOW {
                    #[cfg(windows)]
                    let keep = PROMPT
                        .len()
                        .max(CURSOR_POSITION_REQUEST.len())
                        .saturating_sub(1);
                    #[cfg(not(windows))]
                    let keep = PROMPT.len().saturating_sub(1);
                    let discard = window.len().saturating_sub(keep);
                    window.drain(..discard);
                }
            }
            chunk[..count].zeroize();
        }
    });

    let prompt_deadline = Instant::now() + PROMPT_TIMEOUT;
    let prompted = loop {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(PtyEvent::Prompt) => break true,
            #[cfg(windows)]
            Ok(PtyEvent::CursorPositionRequest) => {
                // portable-pty opens ConPTY with PSEUDOCONSOLE_INHERIT_CURSOR.
                // ConPTY waits for this terminal response before forwarding the
                // CLI prompt, so answer it without mixing it with the secret.
                if writer.write_all(CURSOR_POSITION_RESPONSE).is_err() || writer.flush().is_err() {
                    break false;
                }
                continue;
            }
            Err(_) => {}
        }
        if child.try_wait().ok().flatten().is_some() || Instant::now() >= prompt_deadline {
            break false;
        }
    };
    if !prompted
        || writer.write_all(value.as_bytes()).is_err()
        || writer.write_all(b"\r").is_err()
        || writer.flush().is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        drop(writer);
        let _ = reader_thread.join();
        return false;
    }
    let completion_deadline = Instant::now() + COMPLETION_TIMEOUT;
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < completion_deadline => {
                thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    drop(writer);
    let _ = reader_thread.join();
    success
}

pub(super) fn pty_command(executable: &Path, args: &[OsString]) -> CommandBuilder {
    if cfg!(windows)
        && executable.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        let mut command = CommandBuilder::new("cmd.exe");
        command.args([
            OsString::from("/D"),
            OsString::from("/Q"),
            OsString::from("/C"),
            OsString::from("call"),
        ]);
        command.arg(executable.as_os_str());
        command.args(args.iter());
        command
    } else {
        let mut command = CommandBuilder::new(executable.as_os_str());
        command.args(args.iter());
        command
    }
}

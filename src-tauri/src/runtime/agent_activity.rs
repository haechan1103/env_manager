use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use tempfile::NamedTempFile;

use super::AgentActivityEvent;

const MAX_AUDIT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn load_agent_activity(
    audit_dir: &Path,
    legacy_dir: &Path,
    project_id: &str,
) -> Vec<AgentActivityEvent> {
    let mut events = read_events(&audit_dir.join(format!("{project_id}.jsonl")), project_id);
    for path in [
        legacy_dir.join(format!("{project_id}.previous.jsonl")),
        legacy_dir.join(format!("{project_id}.jsonl")),
    ] {
        events.extend(read_events(&path, project_id));
    }
    events.sort_by_key(|event| std::cmp::Reverse(event.timestamp_ms));
    let mut seen = BTreeSet::new();
    events.retain(|event| seen.insert(event_identity(event)));
    events.truncate(200);
    events
}

pub(super) fn migrate_legacy_agent_activity(
    legacy_dir: &Path,
    audit_dir: &Path,
    project_ids: &[&str],
) -> io::Result<usize> {
    if legacy_dir == audit_dir || !legacy_dir.is_dir() {
        return Ok(0);
    }

    let mut migrated = 0;
    for project_id in project_ids
        .iter()
        .copied()
        .filter(|project_id| valid_project_id(project_id))
    {
        let sources = [
            legacy_dir.join(format!("{project_id}.previous.jsonl")),
            legacy_dir.join(format!("{project_id}.jsonl")),
        ];
        let source_events = sources
            .iter()
            .flat_map(|path| read_events(path, project_id))
            .collect::<Vec<_>>();
        if source_events.is_empty() {
            continue;
        }

        fs::create_dir_all(audit_dir)?;
        let target = audit_dir.join(format!("{project_id}.jsonl"));
        let mut events = read_events(&target, project_id);
        let existing = events.iter().map(event_identity).collect::<BTreeSet<_>>();
        migrated += source_events
            .iter()
            .filter(|event| !existing.contains(&event_identity(event)))
            .count();
        events.extend(source_events);
        events.sort_by_key(|event| event.timestamp_ms);
        let mut seen = BTreeSet::new();
        events.retain(|event| seen.insert(event_identity(event)));
        write_recent_events(audit_dir, &target, &events)?;

        for source in sources {
            match fs::remove_file(source) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => {}
            }
        }
    }
    Ok(migrated)
}

fn read_events(path: &Path, project_id: &str) -> Vec<AgentActivityEvent> {
    read_recent_bytes(path).map_or_else(
        |_| Vec::new(),
        |bytes| {
            bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .filter_map(|line| serde_json::from_slice::<AgentActivityEvent>(line).ok())
                .filter(|event| event.project_id == project_id)
                .collect()
        },
    )
}

fn read_recent_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_AUDIT_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity((length - start) as usize);
    file.read_to_end(&mut bytes)?;
    if start == 0 {
        return Ok(bytes);
    }
    let Some(offset) = bytes.iter().position(|byte| *byte == b'\n') else {
        return Ok(Vec::new());
    };
    Ok(bytes.split_off(offset + 1))
}

fn write_recent_events(
    directory: &Path,
    target: &Path,
    events: &[AgentActivityEvent],
) -> io::Result<()> {
    let mut retained = Vec::new();
    let mut retained_bytes = 0;
    for event in events.iter().rev() {
        let mut line = serde_json::to_vec(event).map_err(io::Error::other)?;
        line.push(b'\n');
        if retained_bytes + line.len() > MAX_AUDIT_BYTES {
            break;
        }
        retained_bytes += line.len();
        retained.push(line);
    }
    retained.reverse();

    let mut temporary = NamedTempFile::new_in(directory)?;
    for line in retained {
        temporary.write_all(&line)?;
    }
    temporary.flush()?;
    if target.exists() {
        fs::remove_file(target)?;
    }
    temporary.persist(target).map_err(|error| error.error)?;
    Ok(())
}

fn event_identity(event: &AgentActivityEvent) -> Vec<u8> {
    serde_json::to_vec(event).unwrap_or_default()
}

fn valid_project_id(project_id: &str) -> bool {
    project_id.len() == 16
        && project_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(project_id: &str, timestamp_ms: u64, actor: &str) -> AgentActivityEvent {
        AgentActivityEvent {
            timestamp_ms,
            project_id: project_id.to_owned(),
            actor: actor.to_owned(),
            category: "structure-inspection".to_owned(),
            operation: "inspect_project".to_owned(),
            relative_paths: Vec::new(),
            variable_names: Vec::new(),
            policy_decision: "redacted".to_owned(),
            outcome: "allowed".to_owned(),
            result_code: "OK".to_owned(),
        }
    }

    fn write_events(path: &Path, events: &[AgentActivityEvent]) {
        let mut bytes = Vec::new();
        for event in events {
            serde_json::to_writer(&mut bytes, event).expect("serialize event");
            bytes.push(b'\n');
        }
        fs::write(path, bytes).expect("write events");
    }

    #[test]
    fn migrates_only_registered_project_activity_and_deduplicates_existing_events() {
        let legacy = tempfile::tempdir().expect("legacy audit");
        let app_data = tempfile::tempdir().expect("app audit");
        let audit_dir = app_data.path().join("agent-activity");
        fs::create_dir_all(&audit_dir).expect("audit directory");
        let project_id = "0123456789abcdef";
        let first = event(project_id, 1, "codex");
        let second = event(project_id, 2, "codex");
        write_events(
            &legacy.path().join(format!("{project_id}.jsonl")),
            &[first.clone(), second.clone()],
        );
        write_events(
            &legacy.path().join("ffffffffffffffff.jsonl"),
            &[event("ffffffffffffffff", 3, "unknown-agent")],
        );
        write_events(
            &audit_dir.join(format!("{project_id}.jsonl")),
            std::slice::from_ref(&first),
        );

        let migrated = migrate_legacy_agent_activity(legacy.path(), &audit_dir, &[project_id])
            .expect("migrate audit");

        assert_eq!(migrated, 1);
        assert_eq!(
            read_events(&audit_dir.join(format!("{project_id}.jsonl")), project_id),
            vec![first, second]
        );
        assert!(!legacy.path().join(format!("{project_id}.jsonl")).exists());
        assert!(legacy.path().join("ffffffffffffffff.jsonl").exists());
    }

    #[test]
    fn rejects_project_ids_that_could_escape_the_audit_directory() {
        let legacy = tempfile::tempdir().expect("legacy audit");
        let app_data = tempfile::tempdir().expect("app audit");
        let migrated = migrate_legacy_agent_activity(
            legacy.path(),
            &app_data.path().join("agent-activity"),
            &["../outside"],
        )
        .expect("ignore invalid id");
        assert_eq!(migrated, 0);
    }

    #[test]
    fn activity_reader_keeps_following_an_older_broker_legacy_path() {
        let legacy = tempfile::tempdir().expect("legacy audit");
        let app_data = tempfile::tempdir().expect("app audit");
        let audit_dir = app_data.path().join("agent-activity");
        let project_id = "0123456789abcdef";
        fs::create_dir_all(&audit_dir).expect("audit directory");
        write_events(
            &audit_dir.join(format!("{project_id}.jsonl")),
            &[event(project_id, 1, "codex")],
        );
        write_events(
            &legacy.path().join(format!("{project_id}.jsonl")),
            &[event(project_id, 2, "codex")],
        );

        let activity = load_agent_activity(&audit_dir, legacy.path(), project_id);

        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].timestamp_ms, 2);
        assert_eq!(activity[1].timestamp_ms, 1);
    }
}

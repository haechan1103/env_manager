use super::*;

pub(super) fn project_preview(entries: &[TeamImportEntry]) -> TeamImportPreview {
    let files = entries
        .iter()
        .map(|entry| TeamImportFileProjection {
            path: to_manifest_path(&entry.source_path),
            target_path: to_manifest_path(&entry.target_path),
            occurrences: entry
                .occurrences
                .iter()
                .map(|occurrence| TeamImportOccurrenceProjection {
                    id: occurrence.id.clone(),
                    key: occurrence.key.clone(),
                    state: occurrence.state,
                    link_id: occurrence.link_id.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let states = files.iter().flat_map(|file| &file.occurrences);
    TeamImportPreview {
        new_count: states
            .clone()
            .filter(|item| item.state == TeamImportOccurrenceState::New)
            .count(),
        unchanged_count: states
            .clone()
            .filter(|item| item.state == TeamImportOccurrenceState::Unchanged)
            .count(),
        conflict_count: states
            .filter(|item| item.state == TeamImportOccurrenceState::Conflict)
            .count(),
        files,
    }
}

pub(super) fn validate_package_links(
    manifest: &Manifest,
    entries: &[TeamImportEntry],
) -> EnvResult<()> {
    for link in &manifest.links {
        let mut values = Vec::new();
        for member in &link.members {
            let occurrence = entries
                .iter()
                .find(|entry| to_manifest_path(&entry.target_path) == member.file)
                .and_then(|entry| {
                    entry
                        .occurrences
                        .iter()
                        .find(|occurrence| occurrence.key == link.key)
                });
            if let Some(occurrence) = occurrence {
                values.push(occurrence.decoded_value.as_str());
            }
        }
        if !values.is_empty() && values.len() != link.members.len() {
            return Err(EnvError::link_member_missing(
                &link.key,
                Path::new("encrypted-share"),
            ));
        }
        if values.windows(2).any(|pair| pair[0] != pair[1]) {
            return Err(EnvError::link_conflict(&link.key));
        }
    }
    Ok(())
}

pub(super) fn expand_linked_conflicts(
    entries: &[TeamImportEntry],
    accepted: &mut BTreeSet<String>,
) {
    let selected_links = entries
        .iter()
        .flat_map(|entry| &entry.occurrences)
        .filter(|occurrence| accepted.contains(&occurrence.id))
        .filter_map(|occurrence| occurrence.link_id.clone())
        .collect::<BTreeSet<_>>();
    for occurrence in entries.iter().flat_map(|entry| &entry.occurrences) {
        if occurrence
            .link_id
            .as_ref()
            .is_some_and(|id| selected_links.contains(id))
            && occurrence.state == TeamImportOccurrenceState::Conflict
        {
            accepted.insert(occurrence.id.clone());
        }
    }
}

pub(super) fn is_managed_import_path(
    relative: &Path,
    normalized: &str,
    manifest: &Manifest,
) -> bool {
    let mut options = DiscoveryOptions::default();
    options
        .ignored_files
        .extend(manifest.scan.ignored_files.iter().cloned());
    options
        .ignored_directories
        .extend(manifest.scan.ignored_directories.iter().cloned());
    !options.ignored_files.contains(normalized)
        && !relative.components().any(|component| {
            options
                .ignored_directories
                .contains(&component.as_os_str().to_string_lossy().into_owned())
        })
}

pub(super) fn occurrence_id(relative: &Path, key: &str) -> String {
    let seed = format!("{}\0{key}", to_manifest_path(relative));
    format!("occ-{}", &blake3::hash(seed.as_bytes()).to_hex()[..16])
}

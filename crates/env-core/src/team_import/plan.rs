use super::*;

impl TeamImportPlan {
    pub fn preview(&self) -> &TeamImportPreview {
        &self.preview
    }

    pub fn apply(self, shared_conflicts: &[String]) -> EnvResult<TeamImportSummary> {
        let known_conflicts = self
            .entries
            .iter()
            .flat_map(|entry| &entry.occurrences)
            .filter(|occurrence| occurrence.state == TeamImportOccurrenceState::Conflict)
            .map(|occurrence| occurrence.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut accepted = BTreeSet::new();
        for id in shared_conflicts {
            if !known_conflicts.contains(id.as_str()) {
                return Err(package_invalid());
            }
            accepted.insert(id.clone());
        }
        expand_linked_conflicts(&self.entries, &mut accepted);
        let changes = build_changes(&self.root, &self.entries, &accepted)?;
        validate_link_invariants(&self.root, &self.manifest, &changes)?;
        apply_entries(&self.root, self.entries, changes, &self.preview, &accepted)
    }

    pub fn remap_file(
        &mut self,
        source_path: &str,
        target_path: &str,
    ) -> EnvResult<TeamImportPreview> {
        let source = validate_package_path(source_path)?;
        let target = validate_package_path(target_path)?;
        let target_normalized = to_manifest_path(&target);
        if !is_managed_import_path(&target, &target_normalized, &self.manifest) {
            return Err(package_invalid());
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.source_path != source && entry.target_path == target)
        {
            return Err(package_invalid());
        }
        let index = self
            .entries
            .iter()
            .position(|entry| entry.source_path == source)
            .ok_or_else(package_invalid)?;
        let package_bytes = Zeroizing::new(self.entries[index].package_bytes.to_vec());
        let replacement = build_entry(&self.root, &self.manifest, source, target, package_bytes)?;
        let previous = std::mem::replace(&mut self.entries[index], replacement);
        if let Err(error) = validate_package_links(&self.manifest, &self.entries) {
            self.entries[index] = previous;
            return Err(error);
        }
        self.preview = project_preview(&self.entries);
        Ok(self.preview.clone())
    }

    pub fn reveal_conflict(
        &self,
        occurrence_id: &str,
        side: TeamImportValueSide,
    ) -> EnvResult<Zeroizing<String>> {
        let (entry, occurrence) = self
            .entries
            .iter()
            .find_map(|entry| {
                entry
                    .occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == occurrence_id)
                    .map(|occurrence| (entry, occurrence))
            })
            .filter(|(_, occurrence)| occurrence.state == TeamImportOccurrenceState::Conflict)
            .ok_or_else(package_invalid)?;
        match side {
            TeamImportValueSide::Shared => Ok(Zeroizing::new(occurrence.decoded_value.to_string())),
            TeamImportValueSide::Local => {
                verify_expected(&self.root, entry)?;
                let current = Zeroizing::new(
                    fs::read(self.root.join(&entry.target_path))
                        .map_err(|error| EnvError::io(&entry.target_path, error))?,
                );
                let document = Document::parse(current.to_vec(), &entry.target_path)?;
                Ok(Zeroizing::new(document.decoded_value(&occurrence.key)?))
            }
        }
    }
}

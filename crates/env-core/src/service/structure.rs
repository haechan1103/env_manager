use super::*;

impl ProjectService {
    pub fn create_group(&self, request: CreateGroupRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let name = sanitize_group(&request.name)?;
        if find_unique_group_index(&loaded.document, &name)?.is_some() {
            return Err(EnvError::invalid("같은 이름의 그룹이 이미 있습니다."));
        }

        let newline = newline(&loaded.document);
        let insert_at = loaded.document.source().len();
        let mut block = String::new();
        if insert_at > content_start(&loaded.document) {
            if loaded.document.source()[insert_at - 1] != b'\n' {
                block.push_str(newline);
            }
            if !loaded.document.source()[..insert_at]
                .ends_with(format!("{newline}{newline}").as_bytes())
            {
                block.push_str(newline);
            }
        }
        block.push_str("# @group ");
        block.push_str(&name);
        block.push_str(newline);

        let proposed = loaded
            .document
            .replace_span(crate::Span::new(insert_at, insert_at), block.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: Vec::new(),
        })
    }

    pub fn rename_group(&self, request: RenameGroupRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let current_index = find_unique_group_index(&loaded.document, &request.current_name)?
            .ok_or_else(|| EnvError::invalid("변경할 그룹을 찾지 못했습니다."))?;
        let new_name = sanitize_group(&request.new_name)?;
        if new_name == request.current_name {
            return Err(EnvError::invalid("현재 그룹 이름과 같습니다."));
        }
        if find_unique_group_index(&loaded.document, &new_name)?.is_some() {
            return Err(EnvError::invalid("같은 이름의 그룹이 이미 있습니다."));
        }
        let Node::GroupDirective {
            name: name_span, ..
        } = loaded.document.nodes()[current_index]
        else {
            unreachable!("group lookup only returns group directives")
        };
        let proposed = loaded.document.replace_span(name_span, new_name.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: Vec::new(),
        })
    }

    pub fn add_variable(&self, request: AddVariableRequest) -> EnvResult<MutationSummary> {
        validate_key(&request.key)?;
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        if loaded
            .document
            .assignments()
            .iter()
            .any(|assignment| assignment.key == request.key)
        {
            return Err(EnvError::duplicate_key(&request.key, &relative));
        }

        let newline = newline(&loaded.document);
        let requested_group = request.group.trim();
        let is_ungrouped = requested_group.is_empty() || requested_group == "기타";
        let normalized_group = if is_ungrouped {
            None
        } else {
            Some(sanitize_group(requested_group)?)
        };
        let existing_group = normalized_group
            .as_deref()
            .map(|name| find_unique_group_index(&loaded.document, name))
            .transpose()?
            .flatten();
        let insert_at = if is_ungrouped {
            first_group_start(&loaded.document).unwrap_or(loaded.document.source().len())
        } else {
            existing_group.map_or(loaded.document.source().len(), |group_index| {
                next_group_start(&loaded.document, group_index)
                    .unwrap_or(loaded.document.source().len())
            })
        };

        let mut block = String::new();
        let has_content_before = insert_at > content_start(&loaded.document);
        if has_content_before && loaded.document.source()[insert_at - 1] != b'\n' {
            block.push_str(newline);
        }
        let previous_has_blank = loaded.document.source()[..insert_at]
            .ends_with(format!("{newline}{newline}").as_bytes());
        if has_content_before && !previous_has_blank {
            block.push_str(newline);
        }
        if existing_group.is_none()
            && let Some(group) = normalized_group
        {
            block.push_str("# @group ");
            block.push_str(&group);
            block.push_str(newline);
            block.push_str(newline);
        }
        for line in &request.description {
            block.push_str("# ");
            block.push_str(&sanitize_comment(line));
            block.push_str(newline);
        }
        block.push_str(&request.key);
        block.push('=');
        block.push_str(&encode_new_value(&request.value));
        block.push_str(newline);

        let proposed = loaded
            .document
            .replace_span(crate::Span::new(insert_at, insert_at), block.as_bytes());
        self.commit_one(relative, loaded.revision, proposed)?;
        self.ensure_policy(&request.key)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

    pub fn delete_variable(&self, request: DeleteVariableRequest) -> EnvResult<MutationSummary> {
        let manifest = ManifestStore::for_root(&self.root).load()?;
        if manifest.link_for(&request.file, &request.key).is_some() {
            return Err(EnvError::invalid(
                "연결된 변수는 먼저 현재 occurrence를 연결에서 분리해야 삭제할 수 있습니다.",
            ));
        }
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let assignment = loaded.document.assignment(&request.key)?;
        let start = attached_comment_start(&loaded.document, assignment.node_index)
            .unwrap_or(assignment.span.start);
        let proposed = loaded
            .document
            .replace_span(crate::Span::new(start, assignment.span.end), b"");
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }

    pub fn move_variable(&self, request: MoveVariableRequest) -> EnvResult<MutationSummary> {
        let relative = PathBuf::from(&request.file);
        let loaded = self.load_document(&relative)?;
        let assignment = loaded.document.assignment(&request.key)?;
        let block_start = attached_comment_start(&loaded.document, assignment.node_index)
            .unwrap_or(assignment.span.start);
        let block_end = assignment.span.end;
        let current_group = group_at(&loaded.document, assignment.node_index);
        let target_group = request.target_group.trim();
        if current_group == target_group {
            return Err(EnvError::invalid("이미 선택한 그룹에 있습니다."));
        }

        let target_original = if target_group.is_empty() || target_group == "기타" {
            loaded
                .document
                .nodes()
                .iter()
                .find_map(|node| match node {
                    Node::GroupDirective { span, .. } => Some(span.start),
                    _ => None,
                })
                .unwrap_or(loaded.document.source().len())
        } else {
            let group_index = find_unique_group_index(&loaded.document, target_group)?
                .ok_or_else(|| EnvError::invalid("이동할 그룹을 찾지 못했습니다."))?;
            next_group_start(&loaded.document, group_index)
                .unwrap_or(loaded.document.source().len())
        };

        let block = loaded.document.source()[block_start..block_end].to_vec();
        let mut proposed = loaded.document.source().to_vec();
        proposed.drain(block_start..block_end);
        let removed_len = block_end - block_start;
        let target = if target_original > block_end {
            target_original - removed_len
        } else {
            target_original
        };
        proposed.splice(target..target, block);
        self.commit_one(relative, loaded.revision, proposed)?;
        Ok(MutationSummary {
            affected_files: vec![request.file],
            keys: vec![request.key],
        })
    }
}

use std::path::PathBuf;

use serde::Serialize;

use crate::{
    Document, EnvError, EnvResult, FileRevision, Node, PlannedFileChange, TransactionPlan,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationSuggestion {
    pub current_marker: String,
    pub group_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreview {
    pub file: String,
    pub suggestions: Vec<MigrationSuggestion>,
    pub summary: String,
}

pub struct MigrationPlan {
    pub preview: MigrationPreview,
    change: PlannedFileChange,
}

impl MigrationPlan {
    pub fn build(
        file: String,
        document: &Document,
        expected_revision: FileRevision,
    ) -> EnvResult<Self> {
        let mut replacements = Vec::new();
        let mut suggestions = Vec::new();

        for node in document.nodes() {
            let Node::Comment { span, content } = node else {
                continue;
            };
            let marker = document.text(*content).trim();
            let Some(group_name) = parse_visual_group(marker) else {
                continue;
            };
            replacements.push((
                *span,
                format!("# @group {group_name}{}", line_ending(document, *span)),
            ));
            suggestions.push(MigrationSuggestion {
                current_marker: format!("# {marker}"),
                group_name,
            });
        }

        if suggestions.is_empty() {
            return Err(EnvError::invalid(
                "안전하게 변환할 수 있는 시각적 그룹 주석을 찾지 못했습니다.",
            ));
        }

        let mut proposed = document.source().to_vec();
        replacements.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        for (span, replacement) in replacements {
            proposed.splice(span.start..span.end, replacement.bytes());
        }
        Document::parse(proposed.clone(), std::path::Path::new(&file))?;

        Ok(Self {
            preview: MigrationPreview {
                file: file.clone(),
                summary: format!(
                    "{}개 그룹 표식을 `# @group` 형식으로 바꿉니다. 변수 값과 순서는 바꾸지 않습니다.",
                    suggestions.len()
                ),
                suggestions,
            },
            change: PlannedFileChange {
                relative_path: PathBuf::from(file),
                expected_revision,
                proposed_bytes: proposed,
            },
        })
    }

    pub fn apply(self, root: &std::path::Path) -> EnvResult<()> {
        TransactionPlan::new(vec![self.change]).commit(root)
    }
}

fn parse_visual_group(marker: &str) -> Option<String> {
    let marker = marker.trim();
    let candidate = marker
        .strip_prefix("===")
        .and_then(|value| value.strip_suffix("==="))
        .or_else(|| {
            marker
                .strip_prefix("---")
                .and_then(|value| value.strip_suffix("---"))
        })
        .or_else(|| {
            marker
                .strip_prefix("**")
                .and_then(|value| value.strip_suffix("**"))
        })
        .or_else(|| {
            marker
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
        })?
        .trim();

    if candidate.is_empty()
        || candidate.len() > 80
        || candidate.starts_with('@')
        || candidate.contains(['\r', '\n', '='])
    {
        return None;
    }
    Some(candidate.to_owned())
}

fn line_ending(document: &Document, span: crate::Span) -> &'static str {
    let bytes = &document.source()[span.start..span.end];
    if bytes.ends_with(b"\r\n") {
        "\r\n"
    } else if bytes.ends_with(b"\n") {
        "\n"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn migration_changes_only_strong_visual_markers() {
        let source =
            b"# === GPT ===\r\nGPT_API_KEY=fake_canary\r\n# keep this note\r\nPORT=fake_3000\r\n";
        let document = Document::parse(source.to_vec(), Path::new(".env.local")).expect("parse");
        let plan = MigrationPlan::build(
            ".env.local".to_owned(),
            &document,
            FileRevision::from_bytes(source),
        )
        .expect("plan");

        assert_eq!(plan.preview.suggestions.len(), 1);
        assert_eq!(plan.preview.suggestions[0].group_name, "GPT");
        assert_eq!(
            plan.change.proposed_bytes,
            b"# @group GPT\r\nGPT_API_KEY=fake_canary\r\n# keep this note\r\nPORT=fake_3000\r\n"
        );
    }

    #[test]
    fn ordinary_description_is_not_migrated() {
        let source = b"# This key is server only\nAPI_KEY=fake_canary\n";
        let document = Document::parse(source.to_vec(), Path::new(".env")).expect("parse");
        let result = MigrationPlan::build(
            ".env".to_owned(),
            &document,
            FileRevision::from_bytes(source),
        );
        assert!(result.is_err());
    }
}

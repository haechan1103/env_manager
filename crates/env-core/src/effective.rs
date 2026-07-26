use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkKind {
    NextJs,
    Vite,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveContext {
    pub framework: FrameworkKind,
    pub mode: String,
    pub working_directory: String,
    #[serde(default)]
    pub process_keys: BTreeSet<String>,
    #[serde(default)]
    pub custom_precedence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveOccurrence {
    pub file: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveProjection {
    pub key: String,
    pub winner: Option<String>,
    pub shadowed: Vec<String>,
    pub reason: String,
    pub confidence: String,
}

pub fn resolve_effective(
    context: &EffectiveContext,
    key: &str,
    occurrences: &[EffectiveOccurrence],
) -> EffectiveProjection {
    if context.process_keys.contains(key) {
        return EffectiveProjection {
            key: key.to_owned(),
            winner: Some("process.env".to_owned()),
            shadowed: occurrences
                .iter()
                .filter(|item| item.key == key)
                .map(|item| item.file.clone())
                .collect(),
            reason: "실행 프로세스에 이미 존재하는 값이 파일보다 우선합니다.".to_owned(),
            confidence: "confirmed-profile".to_owned(),
        };
    }

    let precedence = match context.framework {
        FrameworkKind::NextJs => next_precedence(&context.mode),
        FrameworkKind::Vite => vite_precedence(&context.mode),
        FrameworkKind::Custom => context.custom_precedence.clone(),
    };
    let scoped = occurrences
        .iter()
        .filter(|item| item.key == key)
        .collect::<Vec<_>>();

    let mut ordered = Vec::new();
    for candidate in precedence {
        let full_path = join_working_directory(&context.working_directory, &candidate);
        if scoped.iter().any(|item| item.file == full_path) {
            ordered.push(full_path);
        }
    }

    let winner = ordered.first().cloned();
    let shadowed = ordered.into_iter().skip(1).collect::<Vec<_>>();
    let reason = if winner.is_some() {
        format!(
            "{:?} {} 모드의 확인된 파일 우선순위입니다.",
            context.framework, context.mode
        )
    } else {
        "확인된 프로필에서 이 변수의 적용 occurrence를 찾지 못했습니다.".to_owned()
    };

    EffectiveProjection {
        key: key.to_owned(),
        winner,
        shadowed,
        reason,
        confidence: "confirmed-profile".to_owned(),
    }
}

fn next_precedence(mode: &str) -> Vec<String> {
    let mut files = vec![format!(".env.{mode}.local")];
    if mode != "test" {
        files.push(".env.local".to_owned());
    }
    files.push(format!(".env.{mode}"));
    files.push(".env".to_owned());
    files
}

fn vite_precedence(mode: &str) -> Vec<String> {
    vec![
        format!(".env.{mode}.local"),
        format!(".env.{mode}"),
        ".env.local".to_owned(),
        ".env".to_owned(),
    ]
}

fn join_working_directory(directory: &str, file: &str) -> String {
    let directory = directory.trim_matches('/');
    if directory.is_empty() || directory == "." {
        file.to_owned()
    } else {
        format!("{directory}/{file}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(file: &str) -> EffectiveOccurrence {
        EffectiveOccurrence {
            file: file.to_owned(),
            key: "PORT".to_owned(),
        }
    }

    #[test]
    fn next_development_local_wins() {
        let context = EffectiveContext {
            framework: FrameworkKind::NextJs,
            mode: "development".to_owned(),
            working_directory: "apps/web".to_owned(),
            process_keys: BTreeSet::new(),
            custom_precedence: Vec::new(),
        };
        let projection = resolve_effective(
            &context,
            "PORT",
            &[
                occurrence("apps/web/.env"),
                occurrence("apps/web/.env.local"),
                occurrence("apps/web/.env.development.local"),
            ],
        );
        assert_eq!(
            projection.winner.as_deref(),
            Some("apps/web/.env.development.local")
        );
    }

    #[test]
    fn next_test_skips_plain_local() {
        let context = EffectiveContext {
            framework: FrameworkKind::NextJs,
            mode: "test".to_owned(),
            working_directory: ".".to_owned(),
            process_keys: BTreeSet::new(),
            custom_precedence: Vec::new(),
        };
        let projection = resolve_effective(
            &context,
            "PORT",
            &[occurrence(".env.local"), occurrence(".env")],
        );
        assert_eq!(projection.winner.as_deref(), Some(".env"));
    }
}

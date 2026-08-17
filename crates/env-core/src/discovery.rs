use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{EnvError, EnvResult};

const DEFAULT_EXCLUDED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    "coverage",
    "vendor",
    "Pods",
    "DerivedData",
];

#[derive(Debug, Clone)]
pub struct DiscoveryOptions {
    pub ignored_files: BTreeSet<String>,
    pub ignored_directories: BTreeSet<String>,
    pub max_file_bytes: u64,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            ignored_files: BTreeSet::new(),
            ignored_directories: DEFAULT_EXCLUDED_DIRECTORIES
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

pub fn discover_env_files(root: &Path, options: &DiscoveryOptions) -> EnvResult<Vec<PathBuf>> {
    let root = root
        .canonicalize()
        .map_err(|error| EnvError::io(root, error))?;
    let mut files = Vec::new();
    visit_directory(&root, &root, options, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_directory(
    root: &Path,
    directory: &Path,
    options: &DiscoveryOptions,
    files: &mut Vec<PathBuf>,
) -> EnvResult<()> {
    let entries = fs::read_dir(directory).map_err(|error| EnvError::io(directory, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| EnvError::io(directory, error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| EnvError::io(&path, error))?;

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !options.ignored_directories.contains(&name) {
                visit_directory(root, &path, options, files)?;
            }
            continue;
        }

        if !file_type.is_file() || !is_env_candidate(&entry.file_name().to_string_lossy()) {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|_| EnvError::path_outside(&path))?;
        let relative_string = to_manifest_path(relative);
        if options.ignored_files.contains(&relative_string) {
            continue;
        }
        let size = entry
            .metadata()
            .map_err(|error| EnvError::io(&path, error))?
            .len();
        if size > options.max_file_bytes {
            return Err(EnvError::file_too_large(relative));
        }
        files.push(relative.to_path_buf());
    }
    Ok(())
}

pub fn is_env_candidate(name: &str) -> bool {
    if is_env_template(name) {
        return false;
    }
    name == ".env" || name.starts_with(".env.") || name.ends_with(".env") || name.contains(".env.")
}

fn is_env_template(name: &str) -> bool {
    const TEMPLATE_MARKERS: &[&str] = &["example", "sample", "template", "dist"];
    let segments = name.split('.').filter(|segment| !segment.is_empty());
    segments
        .into_iter()
        .any(|segment| TEMPLATE_MARKERS.contains(&segment))
}

pub(crate) fn to_manifest_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use env_test_support::SyntheticProject;

    use super::*;

    #[test]
    fn discovers_only_supported_files() {
        let project = SyntheticProject::new();
        project.write(".env", "PORT=fake_3000\n");
        project.write(".env.local", "PORT=fake_4000\n");
        project.write("runtime.env", "PORT=fake_runtime\n");
        project.write("runtime.env.staging", "PORT=fake_staging\n");
        project.write(".env.example", "PORT=fake_example\n");
        project.write("sample.env", "PORT=fake_sample\n");
        project.write("runtime.env.template", "PORT=fake_template\n");
        project.write(".envrc", "export PORT=fake_shell\n");
        project.write("apps/web/.env.dev", "PORT=fake_5000\n");
        fs::create_dir_all(project.root().join("node_modules/pkg")).expect("node_modules");
        fs::write(
            project.root().join("node_modules/pkg/.env"),
            "PORT=fake_6000\n",
        )
        .expect("ignored fixture");

        let files =
            discover_env_files(project.root(), &DiscoveryOptions::default()).expect("discovery");
        let paths = files
            .iter()
            .map(|path| to_manifest_path(path))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                ".env",
                ".env.local",
                "apps/web/.env.dev",
                "runtime.env",
                "runtime.env.staging"
            ]
        );
    }

    #[test]
    fn recognizes_data_files_but_not_templates_or_shell_envrc() {
        for supported in [
            ".env",
            ".env.production",
            "runtime.env",
            "runtime.env.production",
            "api.env.local",
        ] {
            assert!(is_env_candidate(supported), "{supported} should be managed");
        }
        for excluded in [
            ".env.example",
            ".env.sample",
            "example.env",
            "api.sample.env",
            "runtime.env.template",
            ".env.dist",
            ".envrc",
        ] {
            assert!(!is_env_candidate(excluded), "{excluded} should be excluded");
        }
    }
}

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use crate::{DiscoveryOptions, EnvError, EnvResult, Manifest, discover_env_files};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Zip,
    Age,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    pub destination: PathBuf,
    pub file_count: usize,
    pub format: ExportFormat,
}

pub fn export_project_env(
    root: &Path,
    manifest: &Manifest,
    destination: &Path,
    passphrase: Option<SecretString>,
) -> EnvResult<ExportSummary> {
    let root = root
        .canonicalize()
        .map_err(|error| EnvError::io(root, error))?;
    let files = discover_for_export(&root, manifest)?;
    let parent = destination
        .parent()
        .ok_or_else(|| EnvError::invalid("내보내기 대상 경로가 올바르지 않습니다."))?;
    fs::create_dir_all(parent).map_err(|error| EnvError::io(parent, error))?;
    if destination.exists() {
        return Err(EnvError::invalid("대상 파일이 이미 존재합니다."));
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".env-manager-export-")
        .tempfile_in(parent)
        .map_err(|error| EnvError::io(parent, error))?;
    let format = if let Some(passphrase) = passphrase {
        let encryptor = age::Encryptor::with_user_passphrase(passphrase);
        let encrypted = encryptor
            .wrap_output(&mut temporary)
            .map_err(|_| EnvError::invalid("암호화 내보내기를 시작하지 못했습니다."))?;
        write_zip(&root, &files, encrypted)?
            .finish()
            .map_err(|error| EnvError::io(destination, error))?;
        ExportFormat::Age
    } else {
        let _ = write_zip(&root, &files, &mut temporary)?;
        ExportFormat::Zip
    };
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| EnvError::io(destination, error))?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| EnvError::io(destination, error.error))?;
    Ok(ExportSummary {
        destination: destination.to_path_buf(),
        file_count: files.len(),
        format,
    })
}

fn write_zip<W: Write>(root: &Path, files: &[PathBuf], output: W) -> EnvResult<W> {
    let mut archive = ZipWriter::new_stream(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    for relative in files {
        let target = safe_export_target(root, relative)?;
        let name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        archive
            .start_file(&name, options)
            .map_err(|_| EnvError::invalid("내보내기 ZIP을 만들지 못했습니다."))?;
        let mut source = File::open(&target).map_err(|error| EnvError::io(relative, error))?;
        io::copy(&mut source, &mut archive).map_err(|error| EnvError::io(relative, error))?;
    }
    archive
        .finish()
        .map(|stream| stream.into_inner())
        .map_err(|_| EnvError::invalid("내보내기 ZIP을 완료하지 못했습니다."))
}

fn discover_for_export(root: &Path, manifest: &Manifest) -> EnvResult<Vec<PathBuf>> {
    let mut options = DiscoveryOptions::default();
    options
        .ignored_files
        .extend(manifest.scan.ignored_files.iter().cloned());
    options
        .ignored_directories
        .extend(manifest.scan.ignored_directories.iter().cloned());
    discover_env_files(root, &options)
}

fn safe_export_target(root: &Path, relative: &Path) -> EnvResult<PathBuf> {
    let target = root.join(relative);
    let metadata = fs::symlink_metadata(&target).map_err(|error| EnvError::io(relative, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(EnvError::invalid(
            "심볼릭 링크나 일반 파일이 아닌 env는 내보낼 수 없습니다.",
        ));
    }
    if metadata.len() > 2 * 1024 * 1024 {
        return Err(EnvError::file_too_large(relative));
    }
    let canonical = target
        .canonicalize()
        .map_err(|error| EnvError::io(relative, error))?;
    if !canonical.starts_with(root) {
        return Err(EnvError::path_outside(relative));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use age::secrecy::SecretString;
    use env_test_support::SyntheticProject;

    use super::*;

    #[test]
    fn plain_export_preserves_relative_paths_and_exact_bytes() {
        let project = SyntheticProject::new();
        project.write(".env.local", "TOKEN=fake_export_canary\n");
        project.write("apps/web/.env.dev", "PORT=fake_3000\n");
        project.write(".env.example", "TOKEN=fake_excluded\n");
        let output = project.root().join("exports/project-env.zip");

        let summary = export_project_env(project.root(), &Manifest::default(), &output, None)
            .expect("plain export");

        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.format, ExportFormat::Zip);
        let mut archive =
            zip::ZipArchive::new(File::open(output).expect("archive")).expect("valid zip");
        let mut value = String::new();
        archive
            .by_name(".env.local")
            .expect("root env")
            .read_to_string(&mut value)
            .expect("read fixture export");
        assert_eq!(value, "TOKEN=fake_export_canary\n");
        assert!(archive.by_name(".env.example").is_err());
    }

    #[test]
    fn export_honors_manifest_file_exclusions() {
        let project = SyntheticProject::new();
        project.write(".env.local", "TOKEN=fake_included\n");
        project.write("apps/web/.env.dev", "TOKEN=fake_excluded\n");
        let output = project.root().join("project-env.zip");
        let mut manifest = Manifest::default();
        manifest
            .scan
            .ignored_files
            .push("apps/web/.env.dev".to_owned());

        let summary = export_project_env(project.root(), &manifest, &output, None)
            .expect("export with exclusions");

        assert_eq!(summary.file_count, 1);
        let mut archive =
            zip::ZipArchive::new(File::open(output).expect("archive")).expect("valid zip");
        assert!(archive.by_name(".env.local").is_ok());
        assert!(archive.by_name("apps/web/.env.dev").is_err());
    }

    #[test]
    fn encrypted_export_is_age_compatible_and_contains_a_zip() {
        let project = SyntheticProject::new();
        project.write(".env.local", "TOKEN=fake_encrypted_export_canary\n");
        let output = project.root().join("project-env.zip.age");
        let passphrase = SecretString::from("fake-human-passphrase-2026".to_owned());

        export_project_env(
            project.root(),
            &Manifest::default(),
            &output,
            Some(passphrase.clone()),
        )
        .expect("encrypted export");

        let encrypted = fs::read(output).expect("encrypted bytes");
        assert!(!String::from_utf8_lossy(&encrypted).contains("fake_encrypted_export_canary"));
        let decryptor = age::Decryptor::new(&encrypted[..]).expect("age file");
        let identity = age::scrypt::Identity::new(passphrase);
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .expect("decrypt");
        let mut zip_bytes = Vec::new();
        reader
            .read_to_end(&mut zip_bytes)
            .expect("read decrypted zip");
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("valid zip");
        let mut value = String::new();
        archive
            .by_name(".env.local")
            .expect("env entry")
            .read_to_string(&mut value)
            .expect("read fixture export");
        assert_eq!(value, "TOKEN=fake_encrypted_export_canary\n");
    }
}

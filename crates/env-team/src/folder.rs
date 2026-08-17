use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use env_core::{EnvError, EnvResult, validate_display_name};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::transport::invalid_transport;
use crate::{
    CapabilityProbe, MAX_ENCRYPTED_TEAM_PACKAGE_BYTES, TeamChannelCapabilities,
    TeamChannelConnection, TeamChannelPackage, TeamChannelTransport, TeamChannelTransportConfig,
};

const DESCRIPTOR_NAME: &str = ".env-manager-channel.json";
const PACKAGES_DIRECTORY: &str = "packages";
const PACKAGE_SUFFIX: &str = ".envshare.age";
const PROTOCOL_VERSION: u32 = 1;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FolderChannelDescriptor {
    protocol_version: u32,
    channel_id: String,
    name: String,
}

pub struct FolderTransport {
    root: PathBuf,
    descriptor: FolderChannelDescriptor,
    packages: PathBuf,
}

impl FolderTransport {
    pub(crate) fn open(root: &Path, expected_channel_id: &str) -> EnvResult<Self> {
        let (root, descriptor, packages) = load_channel(root, Some(expected_channel_id))?;
        Ok(Self {
            root,
            descriptor,
            packages,
        })
    }

    fn connection(&self) -> TeamChannelConnection {
        TeamChannelConnection {
            name: self.descriptor.name.clone(),
            transport: TeamChannelTransportConfig::Folder {
                path: self.root.clone(),
                channel_id: self.descriptor.channel_id.clone(),
            },
        }
    }

    fn resolve_package(&self, package_id: &str) -> EnvResult<PathBuf> {
        if !valid_id(package_id) {
            return Err(invalid_transport("팀 채널 패키지 ID가 올바르지 않습니다."));
        }
        let path = self.packages.join(format!("{package_id}{PACKAGE_SUFFIX}"));
        ensure_regular_file_without_symlink(&path)?;
        let metadata = fs::metadata(&path).map_err(|error| EnvError::io(&path, error))?;
        if metadata.len() > MAX_ENCRYPTED_TEAM_PACKAGE_BYTES {
            return Err(invalid_transport(
                "팀 채널 패키지가 지원 크기를 초과합니다.",
            ));
        }
        Ok(path)
    }
}

impl TeamChannelTransport for FolderTransport {
    fn inspect(&self, probe: CapabilityProbe) -> EnvResult<TeamChannelCapabilities> {
        let readable = fs::read_dir(&self.packages).is_ok();
        let publishable = match probe {
            CapabilityProbe::ReadOnly => None,
            CapabilityProbe::ReadAndPublish => Some(readable && probe_publish(&self.packages)),
        };
        Ok(TeamChannelCapabilities {
            readable,
            publishable,
        })
    }

    fn list_packages(&self) -> EnvResult<Vec<TeamChannelPackage>> {
        let entries =
            fs::read_dir(&self.packages).map_err(|error| EnvError::io(&self.packages, error))?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| EnvError::io(&self.packages, error))?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = file_name.strip_suffix(PACKAGE_SUFFIX) else {
                continue;
            };
            if !valid_id(id) {
                return Err(invalid_transport(
                    "팀 채널 패키지 이름이 올바르지 않습니다.",
                ));
            }
            let path = entry.path();
            ensure_regular_file_without_symlink(&path)?;
            let metadata = fs::metadata(&path).map_err(|error| EnvError::io(&path, error))?;
            if metadata.len() > MAX_ENCRYPTED_TEAM_PACKAGE_BYTES {
                return Err(invalid_transport(
                    "팀 채널 패키지가 지원 크기를 초과합니다.",
                ));
            }
            let modified_at_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |duration| {
                    duration.as_millis().min(u128::from(u64::MAX)) as u64
                });
            result.push(TeamChannelPackage {
                id: id.to_owned(),
                byte_size: metadata.len(),
                modified_at_ms,
            });
        }
        result.sort_by(|left, right| {
            right
                .modified_at_ms
                .cmp(&left.modified_at_ms)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(result)
    }

    fn publish(&self, source: &mut dyn Read) -> EnvResult<TeamChannelPackage> {
        let capabilities = self.inspect(CapabilityProbe::ReadAndPublish)?;
        if capabilities.publishable != Some(true) {
            return Err(invalid_transport("팀 채널에 게시할 권한이 없습니다."));
        }
        let id = format!("pkg_{}", unique_id(&self.root));
        let destination = self.packages.join(format!("{id}{PACKAGE_SUFFIX}"));
        let mut staged = tempfile::Builder::new()
            .prefix(".env-manager-package-")
            .tempfile_in(&self.packages)
            .map_err(|error| EnvError::io(&self.packages, error))?;
        let mut bounded = source.take(MAX_ENCRYPTED_TEAM_PACKAGE_BYTES + 1);
        let byte_size = std::io::copy(&mut bounded, staged.as_file_mut())
            .map_err(|error| EnvError::io(&destination, error))?;
        if byte_size > MAX_ENCRYPTED_TEAM_PACKAGE_BYTES {
            return Err(invalid_transport(
                "팀 채널 패키지가 지원 크기를 초과합니다.",
            ));
        }
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|error| EnvError::io(&destination, error))?;
        staged
            .persist_noclobber(&destination)
            .map_err(|error| EnvError::io(&destination, error.error))?;
        let modified_at_ms = fs::metadata(&destination)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| {
                duration.as_millis().min(u128::from(u64::MAX)) as u64
            });
        Ok(TeamChannelPackage {
            id,
            byte_size,
            modified_at_ms,
        })
    }

    fn fetch(&self, package_id: &str, destination: &mut dyn Write) -> EnvResult<()> {
        let path = self.resolve_package(package_id)?;
        let mut source = fs::File::open(&path).map_err(|error| EnvError::io(&path, error))?;
        std::io::copy(&mut source, destination).map_err(|error| EnvError::io(&path, error))?;
        destination
            .flush()
            .map_err(|error| EnvError::io(&path, error))
    }
}

pub fn connect_folder_transport(
    root: &Path,
    suggested_name: &str,
) -> EnvResult<TeamChannelConnection> {
    let root = canonical_directory(root)?;
    let descriptor_path = root.join(DESCRIPTOR_NAME);
    if descriptor_path.exists() {
        let (_root, descriptor, _packages) = load_channel(&root, None)?;
        return FolderTransport::open(&root, &descriptor.channel_id).map(|item| item.connection());
    }

    validate_display_name(suggested_name)?;
    let packages = root.join(PACKAGES_DIRECTORY);
    fs::create_dir_all(&packages).map_err(|error| EnvError::io(&packages, error))?;
    ensure_directory_without_symlink(&packages)?;
    let descriptor = FolderChannelDescriptor {
        protocol_version: PROTOCOL_VERSION,
        channel_id: format!("channel_{}", unique_id(&root)),
        name: suggested_name.trim().to_owned(),
    };
    persist_descriptor(&descriptor_path, &descriptor)?;
    FolderTransport::open(&root, &descriptor.channel_id).map(|item| item.connection())
}

fn load_channel(
    root: &Path,
    expected_channel_id: Option<&str>,
) -> EnvResult<(PathBuf, FolderChannelDescriptor, PathBuf)> {
    let root = canonical_directory(root)?;
    let descriptor_path = root.join(DESCRIPTOR_NAME);
    ensure_regular_file_without_symlink(&descriptor_path)?;
    let descriptor: FolderChannelDescriptor = serde_json::from_slice(
        &fs::read(&descriptor_path).map_err(|error| EnvError::io(&descriptor_path, error))?,
    )
    .map_err(EnvError::serialization)?;
    if descriptor.protocol_version != PROTOCOL_VERSION
        || !valid_id(&descriptor.channel_id)
        || expected_channel_id.is_some_and(|expected| expected != descriptor.channel_id)
    {
        return Err(invalid_transport(
            "지원하지 않거나 일치하지 않는 팀 채널입니다.",
        ));
    }
    validate_display_name(&descriptor.name)?;
    let packages = root.join(PACKAGES_DIRECTORY);
    ensure_directory_without_symlink(&packages)?;
    Ok((root, descriptor, packages))
}

fn canonical_directory(path: &Path) -> EnvResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|error| EnvError::io(path, error))?;
    ensure_directory_without_symlink(&canonical)?;
    Ok(canonical)
}

fn ensure_directory_without_symlink(path: &Path) -> EnvResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| EnvError::io(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_transport("팀 채널 경로는 일반 폴더여야 합니다."));
    }
    Ok(())
}

fn ensure_regular_file_without_symlink(path: &Path) -> EnvResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| EnvError::io(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_transport(
            "팀 채널에는 일반 파일만 사용할 수 있습니다.",
        ));
    }
    Ok(())
}

fn persist_descriptor(path: &Path, descriptor: &FolderChannelDescriptor) -> EnvResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_transport("팀 채널 경로가 올바르지 않습니다."))?;
    let mut staged = NamedTempFile::new_in(parent).map_err(|error| EnvError::io(parent, error))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), descriptor)
        .map_err(EnvError::serialization)?;
    staged
        .write_all(b"\n")
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|error| EnvError::io(path, error))?;
    staged
        .persist_noclobber(path)
        .map_err(|error| EnvError::io(path, error.error))?;
    Ok(())
}

fn probe_publish(packages: &Path) -> bool {
    let probe = packages.join(format!(".env-manager-probe-{}", unique_id(packages)));
    let created = OpenOptions::new().write(true).create_new(true).open(&probe);
    match created {
        Ok(mut file) => {
            let written = file
                .write_all(b"probe")
                .and_then(|_| file.sync_all())
                .is_ok();
            drop(file);
            written && fs::remove_file(&probe).is_ok()
        }
        Err(_) => false,
    }
}

fn unique_id(path: &Path) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let input = format!("{}:{now}:{}:{sequence}", path.display(), std::process::id());
    blake3::hash(input.as_bytes()).to_hex()[..24].to_owned()
}

fn valid_id(value: &str) -> bool {
    (8..=96).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn transport_contract_is_append_only_and_round_trips_ciphertext() {
        let folder = tempfile::tempdir().expect("channel folder");
        let connection =
            connect_folder_transport(folder.path(), "Development team").expect("connect");
        let transport = crate::open_transport(&connection.transport).expect("transport");
        let first = transport
            .publish(&mut Cursor::new(b"fake ciphertext one"))
            .expect("first publish");
        let second = transport
            .publish(&mut Cursor::new(b"fake ciphertext two"))
            .expect("second publish");

        assert_ne!(first.id, second.id);
        assert_eq!(transport.list_packages().expect("list").len(), 2);
        let mut fetched = Vec::new();
        transport.fetch(&first.id, &mut fetched).expect("fetch");
        assert_eq!(fetched, b"fake ciphertext one");
    }

    #[test]
    fn read_only_inspection_does_not_probe_publish() {
        let folder = tempfile::tempdir().expect("channel folder");
        let connection = connect_folder_transport(folder.path(), "Team").expect("connect");
        let transport = crate::open_transport(&connection.transport).expect("transport");

        let capabilities = transport
            .inspect(CapabilityProbe::ReadOnly)
            .expect("inspect");
        assert!(capabilities.readable);
        assert_eq!(capabilities.publishable, None);
        assert!(
            folder
                .path()
                .join(PACKAGES_DIRECTORY)
                .read_dir()
                .expect("entries")
                .all(|entry| !entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .contains("probe"))
        );
    }

    #[test]
    fn rejects_traversal_shaped_package_ids() {
        let folder = tempfile::tempdir().expect("channel folder");
        let connection = connect_folder_transport(folder.path(), "Team").expect("connect");
        let transport = crate::open_transport(&connection.transport).expect("transport");
        let error = transport
            .fetch("../outside", &mut Vec::new())
            .expect_err("reject traversal");
        assert_eq!(error.code(), env_core::EnvErrorCode::InvalidRequest);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_packages() {
        use std::os::unix::fs::symlink;

        let folder = tempfile::tempdir().expect("channel folder");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        let connection = connect_folder_transport(folder.path(), "Team").expect("connect");
        let package_id = "pkg_12345678";
        symlink(
            outside.path(),
            folder
                .path()
                .join(PACKAGES_DIRECTORY)
                .join(format!("{package_id}{PACKAGE_SUFFIX}")),
        )
        .expect("symlink");
        let transport = crate::open_transport(&connection.transport).expect("transport");

        assert!(transport.list_packages().is_err());
    }
}

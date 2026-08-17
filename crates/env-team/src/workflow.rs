use std::fs::File;
use std::io::{self, Write};

use age::secrecy::SecretString;
use env_core::{
    EnvError, EnvResult, ExportOccurrence, ManifestStore, ProjectService, TeamImportPlan,
    export_project_env, plan_encrypted_team_import,
};

use crate::{MAX_ENCRYPTED_TEAM_PACKAGE_BYTES, TeamChannelTransport, TeamPackagePublishSummary};

struct BoundedWriter<'a, W> {
    destination: &'a mut W,
    remaining: u64,
}

impl<'a, W> BoundedWriter<'a, W> {
    fn new(destination: &'a mut W, limit: u64) -> Self {
        Self {
            destination,
            remaining: limit,
        }
    }
}

impl<W: Write> Write for BoundedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encrypted team package exceeds the supported size",
            ));
        }
        let allowed =
            usize::try_from(self.remaining.min(bytes.len() as u64)).unwrap_or(bytes.len());
        let written = self.destination.write(&bytes[..allowed])?;
        self.remaining = self.remaining.saturating_sub(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.destination.flush()
    }
}

pub fn publish_project_package(
    service: &ProjectService,
    transport: &dyn TeamChannelTransport,
    passphrase: SecretString,
    selection: Option<&[ExportOccurrence]>,
) -> EnvResult<TeamPackagePublishSummary> {
    let temporary = tempfile::tempdir()
        .map_err(|error| EnvError::io(std::path::Path::new("team-package-stage"), error))?;
    let encrypted = temporary.path().join("package.envshare.age");
    let manifest = ManifestStore::for_root(service.root()).load()?;
    let export = export_project_env(
        service.root(),
        &manifest,
        &encrypted,
        Some(passphrase),
        selection,
    )?;
    let mut source = File::open(&encrypted).map_err(|error| EnvError::io(&encrypted, error))?;
    let package = transport.publish(&mut source)?;
    Ok(TeamPackagePublishSummary {
        package_id: package.id,
        file_count: export.file_count,
    })
}

pub fn fetch_team_import_plan(
    service: &ProjectService,
    transport: &dyn TeamChannelTransport,
    package_id: &str,
    passphrase: SecretString,
) -> EnvResult<TeamImportPlan> {
    let mut encrypted = tempfile::NamedTempFile::new()
        .map_err(|error| EnvError::io(std::path::Path::new("team-package-stage"), error))?;
    {
        let mut bounded =
            BoundedWriter::new(encrypted.as_file_mut(), MAX_ENCRYPTED_TEAM_PACKAGE_BYTES);
        transport.fetch(package_id, &mut bounded)?;
    }
    encrypted
        .as_file_mut()
        .sync_all()
        .map_err(|error| EnvError::io(encrypted.path(), error))?;
    let manifest = ManifestStore::for_root(service.root()).load()?;
    plan_encrypted_team_import(service.root(), &manifest, encrypted.path(), passphrase)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::Mutex;

    use age::secrecy::SecretString;
    use env_test_support::SyntheticProject;

    use super::*;
    use crate::{CapabilityProbe, TeamChannelCapabilities, TeamChannelPackage};

    #[derive(Default)]
    struct MemoryTransport {
        ciphertext: Mutex<Vec<u8>>,
    }

    impl TeamChannelTransport for MemoryTransport {
        fn inspect(&self, probe: CapabilityProbe) -> EnvResult<TeamChannelCapabilities> {
            Ok(TeamChannelCapabilities {
                readable: true,
                publishable: matches!(probe, CapabilityProbe::ReadAndPublish).then_some(true),
            })
        }

        fn list_packages(&self) -> EnvResult<Vec<TeamChannelPackage>> {
            let ciphertext = self.ciphertext.lock().expect("ciphertext lock");
            Ok((!ciphertext.is_empty())
                .then(|| TeamChannelPackage {
                    id: "pkg_fake_memory".to_owned(),
                    byte_size: ciphertext.len() as u64,
                    modified_at_ms: 0,
                })
                .into_iter()
                .collect())
        }

        fn publish(&self, source: &mut dyn Read) -> EnvResult<TeamChannelPackage> {
            let mut ciphertext = self.ciphertext.lock().expect("ciphertext lock");
            ciphertext.clear();
            source
                .read_to_end(&mut ciphertext)
                .map_err(|error| EnvError::io(std::path::Path::new("memory-transport"), error))?;
            Ok(TeamChannelPackage {
                id: "pkg_fake_memory".to_owned(),
                byte_size: ciphertext.len() as u64,
                modified_at_ms: 0,
            })
        }

        fn fetch(&self, package_id: &str, destination: &mut dyn Write) -> EnvResult<()> {
            if package_id != "pkg_fake_memory" {
                return Err(EnvError::invalid("synthetic package not found"));
            }
            let ciphertext = self.ciphertext.lock().expect("ciphertext lock");
            destination
                .write_all(&ciphertext)
                .map_err(|error| EnvError::io(std::path::Path::new("memory-transport"), error))
        }
    }

    #[test]
    fn encrypted_workflow_reuses_core_export_and_import_without_plaintext_transport() {
        let source = SyntheticProject::new();
        source.write(".env.local", "TOKEN=fake_team_value\n");
        let source_service = ProjectService::open(source.root()).expect("source service");
        source_service.initialize().expect("source initialize");
        let receiver = SyntheticProject::new();
        let receiver_service = ProjectService::open(receiver.root()).expect("receiver service");
        receiver_service.initialize().expect("receiver initialize");
        let transport = MemoryTransport::default();
        let passphrase = "fake-team-passphrase-2026";

        let published = publish_project_package(
            &source_service,
            &transport,
            SecretString::from(passphrase.to_owned()),
            None,
        )
        .expect("publish");
        let plan = fetch_team_import_plan(
            &receiver_service,
            &transport,
            &published.package_id,
            SecretString::from(passphrase.to_owned()),
        )
        .expect("plan");

        assert_eq!(published.file_count, 1);
        assert_eq!(plan.preview().new_count, 1);
    }

    #[test]
    fn inbound_ciphertext_writer_stops_at_the_shared_limit() {
        let mut destination = Vec::new();
        let mut writer = BoundedWriter::new(&mut destination, 4);

        writer.write_all(b"fake").expect("within limit");
        let error = writer
            .write_all(b"overflow")
            .expect_err("reject excess ciphertext");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(destination, b"fake");
    }
}

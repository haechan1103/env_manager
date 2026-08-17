use std::io::{Read, Write};

use env_core::{EnvError, EnvResult};

use crate::{
    CapabilityProbe, TeamChannelCapabilities, TeamChannelPackage, TeamChannelTransportConfig,
};

pub const MAX_ENCRYPTED_TEAM_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;

pub trait TeamChannelTransport: Send + Sync {
    fn inspect(&self, probe: CapabilityProbe) -> EnvResult<TeamChannelCapabilities>;
    fn list_packages(&self) -> EnvResult<Vec<TeamChannelPackage>>;
    fn publish(&self, source: &mut dyn Read) -> EnvResult<TeamChannelPackage>;
    fn fetch(&self, package_id: &str, destination: &mut dyn Write) -> EnvResult<()>;
}

pub fn open_transport(
    config: &TeamChannelTransportConfig,
) -> EnvResult<Box<dyn TeamChannelTransport>> {
    match config {
        TeamChannelTransportConfig::Folder { path, channel_id } => Ok(Box::new(
            crate::folder::FolderTransport::open(path, channel_id)?,
        )),
    }
}

pub(crate) fn invalid_transport(message: impl Into<String>) -> EnvError {
    EnvError::invalid(message)
}

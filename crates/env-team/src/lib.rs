mod domain;
mod folder;
mod transport;
mod workflow;

pub use domain::{
    CapabilityProbe, TeamChannelCapabilities, TeamChannelConnection, TeamChannelPackage,
    TeamChannelRegistration, TeamChannelTransportConfig, TeamPackagePublishSummary,
    deserialize_team_channel_registrations, local_registration_id,
    registry_contains_legacy_team_channels,
};
pub use folder::connect_folder_transport;
pub use transport::{MAX_ENCRYPTED_TEAM_PACKAGE_BYTES, TeamChannelTransport, open_transport};
pub use workflow::{fetch_team_import_plan, publish_project_package};

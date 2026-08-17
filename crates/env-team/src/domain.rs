use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityProbe {
    ReadOnly,
    ReadAndPublish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamChannelCapabilities {
    pub readable: bool,
    pub publishable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamChannelPackage {
    pub id: String,
    pub byte_size: u64,
    pub modified_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TeamChannelTransportConfig {
    Folder {
        path: PathBuf,
        #[serde(alias = "channel_id")]
        channel_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamChannelConnection {
    pub name: String,
    pub transport: TeamChannelTransportConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamChannelRegistration {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub transport: TeamChannelTransportConfig,
}

pub fn local_registration_id(project_id: &str, transport: &TeamChannelTransportConfig) -> String {
    match transport {
        TeamChannelTransportConfig::Folder { channel_id, .. } => {
            let digest = blake3::hash(format!("{project_id}:{channel_id}").as_bytes());
            // Keep the original prefix and hash input so existing Folder registrations remain stable.
            format!("folder_{}", &digest.to_hex()[..24])
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyFolderTeamChannelRegistration {
    id: String,
    project_id: String,
    channel_id: String,
    name: String,
    root: PathBuf,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredTeamChannelRegistration {
    Current(TeamChannelRegistration),
    Legacy(LegacyFolderTeamChannelRegistration),
}

pub fn deserialize_team_channel_registrations<'de, D>(
    deserializer: D,
) -> Result<Vec<TeamChannelRegistration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let stored = Vec::<StoredTeamChannelRegistration>::deserialize(deserializer)?;
    Ok(stored
        .into_iter()
        .map(|registration| match registration {
            StoredTeamChannelRegistration::Current(registration) => registration,
            StoredTeamChannelRegistration::Legacy(legacy) => TeamChannelRegistration {
                id: legacy.id,
                project_id: legacy.project_id,
                name: legacy.name,
                transport: TeamChannelTransportConfig::Folder {
                    path: legacy.root,
                    channel_id: legacy.channel_id,
                },
            },
        })
        .collect())
}

pub fn registry_contains_legacy_team_channels(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|registry| {
            registry
                .get("teamChannels")
                .and_then(serde_json::Value::as_array)
                .map(|channels| {
                    channels.iter().any(|channel| {
                        channel.get("transport").is_none()
                            && channel.get("root").is_some()
                            && channel.get("channelId").is_some()
                    })
                })
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPackagePublishSummary {
    pub package_id: String,
    pub file_count: usize,
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RegistryFixture {
        #[serde(default)]
        #[serde(deserialize_with = "deserialize_team_channel_registrations")]
        team_channels: Vec<TeamChannelRegistration>,
    }

    #[test]
    fn legacy_folder_registration_migrates_to_tagged_transport_config() {
        let legacy = br#"{
          "teamChannels": [{
            "id": "folder_fake_registration",
            "projectId": "fake-project",
            "channelId": "channel_fake_shared_id",
            "name": "Synthetic channel",
            "root": "/synthetic/team-share"
          }]
        }"#;

        assert!(registry_contains_legacy_team_channels(legacy));
        let registry: RegistryFixture = serde_json::from_slice(legacy).expect("legacy registry");
        assert_eq!(
            registry.team_channels[0].transport,
            TeamChannelTransportConfig::Folder {
                path: PathBuf::from("/synthetic/team-share"),
                channel_id: "channel_fake_shared_id".to_owned(),
            }
        );

        let migrated = serde_json::to_value(registry).expect("migrated registry");
        let channel = &migrated["teamChannels"][0];
        assert_eq!(channel["transport"]["type"], "folder");
        assert_eq!(
            channel["transport"]["path"],
            serde_json::Value::String("/synthetic/team-share".to_owned())
        );
        assert!(channel.get("root").is_none());
        assert!(channel.get("channelId").is_none());
    }

    #[test]
    fn current_registration_is_not_reported_as_legacy() {
        let current = br#"{
          "teamChannels": [{
            "id": "folder_fake_registration",
            "projectId": "fake-project",
            "name": "Synthetic channel",
            "transport": {
              "type": "folder",
              "path": "/synthetic/team-share",
              "channelId": "channel_fake_shared_id"
            }
          }]
        }"#;

        assert!(!registry_contains_legacy_team_channels(current));
        let registry: RegistryFixture = serde_json::from_slice(current).expect("current registry");
        let serialized = serde_json::to_value(registry).expect("serialized registry");
        let transport = &serialized["teamChannels"][0]["transport"];
        assert_eq!(transport["channelId"], "channel_fake_shared_id");
        assert!(transport.get("channel_id").is_none());
    }

    #[test]
    fn folder_registration_id_preserves_the_existing_stable_scheme() {
        let config = TeamChannelTransportConfig::Folder {
            path: PathBuf::from("/synthetic/team-share"),
            channel_id: "channel_fake_shared_id".to_owned(),
        };

        let first = local_registration_id("fake-project", &config);
        let second = local_registration_id("fake-project", &config);

        assert_eq!(first, second);
        assert!(first.starts_with("folder_"));
    }
}

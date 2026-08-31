use std::path::Path;

use env_core::{EnvError, EnvErrorCode, EnvResult, ProjectService};
use env_team::{
    CapabilityProbe, TeamChannelPackage, TeamChannelRegistration, TeamChannelTransport,
};
use serde::Serialize;

use super::{AppRuntime, TeamImportPlanProjection};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamChannelProjection {
    pub id: String,
    pub name: String,
    pub readable: bool,
    pub publishable: bool,
    pub packages: Vec<TeamChannelPackage>,
}

impl AppRuntime {
    pub fn connect_folder_team_channel(
        &self,
        project_id: &str,
        root: &Path,
        suggested_name: &str,
    ) -> EnvResult<TeamChannelProjection> {
        self.root(project_id)?;
        let connection = env_team::connect_folder_transport(root, suggested_name)?;
        let local_id = env_team::local_registration_id(project_id, &connection.transport);
        let registration = TeamChannelRegistration {
            id: local_id.clone(),
            project_id: project_id.to_owned(),
            name: connection.name,
            transport: connection.transport,
        };
        self.update_registry(|registry| {
            if let Some(existing) = registry
                .team_channels
                .iter_mut()
                .find(|channel| channel.id == local_id)
            {
                *existing = registration;
            } else {
                registry.team_channels.push(registration);
            }
            Ok(())
        })?;
        self.team_channel(project_id, &local_id)
    }

    pub fn list_team_channels(&self, project_id: &str) -> EnvResult<Vec<TeamChannelProjection>> {
        self.root(project_id)?;
        self.refresh_registry()?;
        let registrations = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .team_channels
            .iter()
            .filter(|channel| channel.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        let mut channels = registrations
            .iter()
            .map(project_team_channel)
            .collect::<EnvResult<Vec<_>>>()?;
        channels.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        Ok(channels)
    }

    pub fn remove_team_channel(&self, project_id: &str, registration_id: &str) -> EnvResult<()> {
        self.update_registry(|registry| {
            let before = registry.team_channels.len();
            registry.team_channels.retain(|channel| {
                channel.project_id != project_id || channel.id != registration_id
            });
            if before == registry.team_channels.len() {
                return Err(EnvError::invalid("등록된 팀 채널을 찾지 못했습니다."));
            }
            Ok(())
        })
    }

    pub fn prepare_team_channel_operation(
        &self,
        project_id: &str,
        registration_id: &str,
    ) -> EnvResult<(ProjectService, Box<dyn TeamChannelTransport>)> {
        let registration = self.team_channel_registration(project_id, registration_id)?;
        Ok((
            self.service(project_id)?,
            env_team::open_transport(&registration.transport)?,
        ))
    }

    pub fn plan_team_channel_import(
        &self,
        project_id: &str,
        registration_id: &str,
        package_id: &str,
        passphrase: age::secrecy::SecretString,
    ) -> EnvResult<TeamImportPlanProjection> {
        let (service, transport) =
            self.prepare_team_channel_operation(project_id, registration_id)?;
        let plan =
            env_team::fetch_team_import_plan(&service, transport.as_ref(), package_id, passphrase)?;
        self.store_team_import_plan(project_id, plan)
    }

    fn team_channel(
        &self,
        project_id: &str,
        registration_id: &str,
    ) -> EnvResult<TeamChannelProjection> {
        let registration = self.team_channel_registration(project_id, registration_id)?;
        project_team_channel(&registration)
    }

    fn team_channel_registration(
        &self,
        project_id: &str,
        registration_id: &str,
    ) -> EnvResult<TeamChannelRegistration> {
        self.root(project_id)?;
        self.refresh_registry()?;
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .team_channels
            .iter()
            .find(|channel| channel.project_id == project_id && channel.id == registration_id)
            .cloned()
            .ok_or_else(|| EnvError::invalid("등록된 팀 채널을 찾지 못했습니다."))
    }
}

fn project_team_channel(
    registration: &TeamChannelRegistration,
) -> EnvResult<TeamChannelProjection> {
    let transport = match env_team::open_transport(&registration.transport) {
        Ok(transport) => transport,
        Err(error) if error.code() == EnvErrorCode::Io => {
            return Ok(TeamChannelProjection {
                id: registration.id.clone(),
                name: registration.name.clone(),
                readable: false,
                publishable: false,
                packages: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };
    let capabilities = transport.inspect(CapabilityProbe::ReadAndPublish)?;
    let packages = if capabilities.readable {
        transport.list_packages()?
    } else {
        Vec::new()
    };
    Ok(TeamChannelProjection {
        id: registration.id.clone(),
        name: registration.name.clone(),
        readable: capabilities.readable,
        publishable: capabilities.publishable == Some(true),
        packages,
    })
}

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use env_core::{
    AddVariableRequest, CodexAccess, CreateGroupRequest, DeleteVariableRequest, EnvError,
    GitignoreUpdateSummary, LinkRequest, MoveVariableRequest, MutationSummary, ProjectProjection,
    RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
};
use env_credentials::{
    AccountProjection, AccountSecretField, CreateAccountInput, CredentialError, UpdateAccountInput,
};
use env_provider::action_pack::{self, ActionExecutionRequest};
use env_provider::provider_push::{self, ProviderCompareRequest, ProviderPushRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{
    AgentActivityEvent, AppRuntime, CredentialRuntime, MigrationPlanProjection, ProjectSummary,
    ProviderPushReceipt, TeamChannelProjection,
};
use crate::{integrations, integrations::AgentIntegrationId};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    code: String,
    message: String,
}

impl From<EnvError> for CommandError {
    fn from(error: EnvError) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
        }
    }
}

impl From<integrations::IntegrationError> for CommandError {
    fn from(error: integrations::IntegrationError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

impl From<provider_push::ProviderPushError> for CommandError {
    fn from(error: provider_push::ProviderPushError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

impl From<action_pack::ActionPackError> for CommandError {
    fn from(error: action_pack::ActionPackError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message.to_owned(),
        }
    }
}

impl From<CredentialError> for CommandError {
    fn from(error: CredentialError) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
        }
    }
}

type CommandResult<T> = Result<T, CommandError>;

fn provider_task_interrupted() -> CommandError {
    CommandError {
        code: "PROVIDER_TASK_INTERRUPTED".to_owned(),
        message: "배포 서비스 확인 작업이 중단되었습니다.".to_owned(),
    }
}

fn provider_app_data(app: &AppHandle) -> CommandResult<std::path::PathBuf> {
    app.path().app_data_dir().map_err(|_| CommandError {
        code: "PROVIDER_ADAPTER_STORAGE_UNAVAILABLE".to_owned(),
        message: "Provider Adapter 저장 위치를 확인하지 못했습니다.".to_owned(),
    })
}

mod accounts;
mod agent_tools;
mod env_files;
mod projects;
mod providers;
mod sharing;

pub use accounts::*;
pub use agent_tools::*;
pub use env_files::*;
pub use projects::*;
pub use providers::*;
pub use sharing::*;

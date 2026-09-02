use std::path::Path;
use std::sync::Arc;

use env_credentials::{
    AccountProjection, AccountSecretField, CreateAccountInput, CredentialError, CredentialVault,
    NativeSecretStore, UpdateAccountInput,
};

pub struct CredentialRuntime {
    vault: Result<CredentialVault, CredentialError>,
}

impl CredentialRuntime {
    pub fn load(app_data: &Path) -> Self {
        let vault = NativeSecretStore::new()
            .map(|store| CredentialVault::new(app_data.join("credentials.json"), Arc::new(store)));
        Self { vault }
    }

    fn vault(&self) -> Result<&CredentialVault, CredentialError> {
        self.vault.as_ref().map_err(|error| {
            CredentialError::new(error.code(), "운영체제 보안 저장소를 사용할 수 없습니다.")
        })
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<AccountProjection>, CredentialError> {
        self.vault()?.list_for_project(project_id)
    }

    pub fn create(&self, input: CreateAccountInput) -> Result<AccountProjection, CredentialError> {
        self.vault()?.create(input)
    }

    pub fn update(&self, input: UpdateAccountInput) -> Result<(), CredentialError> {
        self.vault()?.update(input)
    }

    pub fn delete(&self, account_id: &str) -> Result<(), CredentialError> {
        self.vault()?.delete(account_id)
    }

    pub fn set_project_access(
        &self,
        account_id: &str,
        project_id: &str,
        allowed: bool,
    ) -> Result<(), CredentialError> {
        self.vault()?
            .set_project_access(account_id, project_id, allowed)
    }

    pub fn revoke_project(&self, project_id: &str) -> Result<(), CredentialError> {
        self.vault()?.revoke_project(project_id)
    }

    pub fn secret_field(
        &self,
        account_id: &str,
        project_id: &str,
        field: AccountSecretField,
    ) -> Result<zeroize::Zeroizing<String>, CredentialError> {
        self.vault()?.secret_field(account_id, project_id, field)
    }
}

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::NamedTempFile;
use zeroize::Zeroizing;

use crate::error::{CredentialError, CredentialResult};
use crate::model::{
    AccountMetadata, AccountProjection, AccountSecret, AccountSecretField, CreateAccountInput,
    MetadataRegistry, UpdateAccountInput,
};

const MAX_METADATA_BYTES: u64 = 1024 * 1024;
// Windows Credential Manager caps a generic credential blob at 5 * 512 bytes.
// Keep the cross-platform envelope below that same limit on every platform.
const MAX_SECRET_BYTES: usize = 5 * 512;
const SECRET_MAGIC: &[u8; 4] = b"KVC1";

pub trait SecretStore: Send + Sync {
    fn put(&self, account_id: &str, secret: &[u8]) -> CredentialResult<()>;
    fn get(&self, account_id: &str) -> CredentialResult<Zeroizing<Vec<u8>>>;
    fn delete(&self, account_id: &str) -> CredentialResult<()>;
}

pub struct CredentialVault {
    metadata_path: PathBuf,
    secret_store: Arc<dyn SecretStore>,
    mutation: Mutex<()>,
}

impl CredentialVault {
    pub fn new(metadata_path: PathBuf, secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            metadata_path,
            secret_store,
            mutation: Mutex::new(()),
        }
    }

    pub fn list_for_project(&self, project_id: &str) -> CredentialResult<Vec<AccountProjection>> {
        validate_project_id(project_id)?;
        let mut accounts = read_registry(&self.metadata_path)?
            .accounts
            .into_iter()
            .map(|account| AccountProjection {
                allowed_for_project: account.granted_project_ids.contains(project_id),
                allowed_project_count: account.granted_project_ids.len(),
                id: account.id,
                display_name: account.display_name,
                service: account.service,
                created_at_ms: account.created_at_ms,
                updated_at_ms: account.updated_at_ms,
            })
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| {
            left.display_name
                .to_ascii_lowercase()
                .cmp(&right.display_name.to_ascii_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(accounts)
    }

    pub fn create(&self, input: CreateAccountInput) -> CredentialResult<AccountProjection> {
        validate_label(&input.display_name, 120)?;
        validate_label(&input.service, 240)?;
        validate_secret(&input.username, 512)?;
        validate_secret(&input.password, 2_000)?;
        if let Some(project_id) = input.grant_project_id.as_deref() {
            validate_project_id(project_id)?;
        }

        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut registry = read_registry(&self.metadata_path)?;
        let id = random_account_id()?;
        let secret = encode_secret(&input.username, &input.password)?;
        self.secret_store.put(&id, &secret)?;

        let now = now_ms();
        let mut granted_project_ids = std::collections::BTreeSet::new();
        if let Some(project_id) = input.grant_project_id.as_ref() {
            granted_project_ids.insert(project_id.clone());
        }
        let metadata = AccountMetadata {
            id: id.clone(),
            display_name: input.display_name.trim().to_owned(),
            service: input.service.trim().to_owned(),
            granted_project_ids,
            created_at_ms: now,
            updated_at_ms: now,
        };
        registry.accounts.push(metadata.clone());
        if let Err(error) = write_registry(&self.metadata_path, &registry) {
            let _ = self.secret_store.delete(&id);
            return Err(error);
        }
        Ok(project_projection(
            metadata,
            input.grant_project_id.as_deref(),
        ))
    }

    pub fn update(&self, input: UpdateAccountInput) -> CredentialResult<()> {
        validate_account_id(&input.account_id)?;
        validate_label(&input.display_name, 120)?;
        validate_label(&input.service, 240)?;
        if let Some(username) = &input.username {
            validate_secret(username, 512)?;
        }
        if let Some(password) = &input.password {
            validate_secret(password, 2_000)?;
        }

        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut registry = read_registry(&self.metadata_path)?;
        let metadata = registry
            .accounts
            .iter_mut()
            .find(|account| account.id == input.account_id)
            .ok_or_else(CredentialError::not_found)?;

        let old_secret = if input.username.is_some() || input.password.is_some() {
            Some(self.secret_store.get(&input.account_id)?)
        } else {
            None
        };
        if let Some(old_secret) = old_secret.as_ref() {
            let decoded = decode_secret(old_secret)?;
            let username = input.username.as_ref().unwrap_or(&decoded.username);
            let password = input.password.as_ref().unwrap_or(&decoded.password);
            let replacement = encode_secret(username, password)?;
            self.secret_store.put(&input.account_id, &replacement)?;
        }

        metadata.display_name = input.display_name.trim().to_owned();
        metadata.service = input.service.trim().to_owned();
        metadata.updated_at_ms = now_ms();
        if let Err(error) = write_registry(&self.metadata_path, &registry) {
            if let Some(old_secret) = old_secret.as_ref() {
                let _ = self.secret_store.put(&input.account_id, old_secret);
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn delete(&self, account_id: &str) -> CredentialResult<()> {
        validate_account_id(account_id)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut registry = read_registry(&self.metadata_path)?;
        if !registry
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            return Err(CredentialError::not_found());
        }
        let old_secret = self.secret_store.get(account_id)?;
        self.secret_store.delete(account_id)?;
        registry.accounts.retain(|account| account.id != account_id);
        if let Err(error) = write_registry(&self.metadata_path, &registry) {
            let _ = self.secret_store.put(account_id, &old_secret);
            return Err(error);
        }
        Ok(())
    }

    pub fn set_project_access(
        &self,
        account_id: &str,
        project_id: &str,
        allowed: bool,
    ) -> CredentialResult<()> {
        validate_account_id(account_id)?;
        validate_project_id(project_id)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut registry = read_registry(&self.metadata_path)?;
        let account = registry
            .accounts
            .iter_mut()
            .find(|account| account.id == account_id)
            .ok_or_else(CredentialError::not_found)?;
        if allowed {
            account.granted_project_ids.insert(project_id.to_owned());
        } else {
            account.granted_project_ids.remove(project_id);
        }
        account.updated_at_ms = now_ms();
        write_registry(&self.metadata_path, &registry)
    }

    pub fn revoke_project(&self, project_id: &str) -> CredentialResult<()> {
        validate_project_id(project_id)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut registry = read_registry(&self.metadata_path)?;
        let mut changed = false;
        for account in &mut registry.accounts {
            if account.granted_project_ids.remove(project_id) {
                account.updated_at_ms = now_ms();
                changed = true;
            }
        }
        if changed {
            write_registry(&self.metadata_path, &registry)?;
        }
        Ok(())
    }

    pub fn secret_field(
        &self,
        account_id: &str,
        project_id: &str,
        field: AccountSecretField,
    ) -> CredentialResult<Zeroizing<String>> {
        validate_account_id(account_id)?;
        validate_project_id(project_id)?;
        let registry = read_registry(&self.metadata_path)?;
        let account = registry
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .ok_or_else(CredentialError::not_found)?;
        if !account.granted_project_ids.contains(project_id) {
            return Err(CredentialError::project_not_allowed());
        }
        let secret = decode_secret(&self.secret_store.get(account_id)?)?;
        Ok(match field {
            AccountSecretField::Username => secret.username,
            AccountSecretField::Password => secret.password,
        })
    }
}

fn project_projection(metadata: AccountMetadata, project_id: Option<&str>) -> AccountProjection {
    AccountProjection {
        allowed_for_project: project_id
            .is_some_and(|project_id| metadata.granted_project_ids.contains(project_id)),
        allowed_project_count: metadata.granted_project_ids.len(),
        id: metadata.id,
        display_name: metadata.display_name,
        service: metadata.service,
        created_at_ms: metadata.created_at_ms,
        updated_at_ms: metadata.updated_at_ms,
    }
}

fn encode_secret(username: &str, password: &str) -> CredentialResult<Zeroizing<Vec<u8>>> {
    if 12 + username.len() + password.len() > MAX_SECRET_BYTES {
        return Err(CredentialError::invalid());
    }
    let username_length = u32::try_from(username.len()).map_err(|_| CredentialError::invalid())?;
    let password_length = u32::try_from(password.len()).map_err(|_| CredentialError::invalid())?;
    let mut encoded = Zeroizing::new(Vec::with_capacity(12 + username.len() + password.len()));
    encoded.extend_from_slice(SECRET_MAGIC);
    encoded.extend_from_slice(&username_length.to_be_bytes());
    encoded.extend_from_slice(&password_length.to_be_bytes());
    encoded.extend_from_slice(username.as_bytes());
    encoded.extend_from_slice(password.as_bytes());
    Ok(encoded)
}

fn decode_secret(encoded: &[u8]) -> CredentialResult<AccountSecret> {
    if encoded.len() < 12 || encoded.len() > MAX_SECRET_BYTES || &encoded[..4] != SECRET_MAGIC {
        return Err(CredentialError::secret_missing());
    }
    let username_length = u32::from_be_bytes(
        encoded[4..8]
            .try_into()
            .map_err(|_| CredentialError::secret_missing())?,
    ) as usize;
    let password_length = u32::from_be_bytes(
        encoded[8..12]
            .try_into()
            .map_err(|_| CredentialError::secret_missing())?,
    ) as usize;
    let username_end = 12usize
        .checked_add(username_length)
        .ok_or_else(CredentialError::secret_missing)?;
    let password_end = username_end
        .checked_add(password_length)
        .ok_or_else(CredentialError::secret_missing)?;
    if password_end != encoded.len() {
        return Err(CredentialError::secret_missing());
    }
    let username = String::from_utf8(encoded[12..username_end].to_vec())
        .map_err(|_| CredentialError::secret_missing())?;
    let password = String::from_utf8(encoded[username_end..password_end].to_vec())
        .map_err(|_| CredentialError::secret_missing())?;
    Ok(AccountSecret {
        username: Zeroizing::new(username),
        password: Zeroizing::new(password),
    })
}

fn read_registry(path: &Path) -> CredentialResult<MetadataRegistry> {
    let parent = path.parent().ok_or_else(CredentialError::metadata_failed)?;
    fs::create_dir_all(parent).map_err(|_| CredentialError::metadata_failed())?;
    let lock_path = parent.join("credentials.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| CredentialError::metadata_failed())?;
    lock.lock_shared()
        .map_err(|_| CredentialError::metadata_failed())?;
    let result = read_registry_unlocked(path);
    let unlock = lock
        .unlock()
        .map_err(|_| CredentialError::metadata_failed());
    match (result, unlock) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(registry), Ok(())) => Ok(registry),
    }
}

fn read_registry_unlocked(path: &Path) -> CredentialResult<MetadataRegistry> {
    if !path.exists() {
        return Ok(MetadataRegistry::default());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| CredentialError::metadata_failed())?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err(CredentialError::metadata_failed());
    }
    let bytes = fs::read(path).map_err(|_| CredentialError::metadata_failed())?;
    let registry = serde_json::from_slice::<MetadataRegistry>(&bytes)
        .map_err(|_| CredentialError::metadata_failed())?;
    if registry.schema_version != 1 || registry.accounts.len() > 500 {
        return Err(CredentialError::metadata_failed());
    }
    Ok(registry)
}

fn write_registry(path: &Path, registry: &MetadataRegistry) -> CredentialResult<()> {
    let parent = path.parent().ok_or_else(CredentialError::metadata_failed)?;
    fs::create_dir_all(parent).map_err(|_| CredentialError::metadata_failed())?;
    let lock_path = parent.join("credentials.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| CredentialError::metadata_failed())?;
    lock.lock()
        .map_err(|_| CredentialError::metadata_failed())?;
    let result = (|| {
        let mut staged =
            NamedTempFile::new_in(parent).map_err(|_| CredentialError::metadata_failed())?;
        serde_json::to_writer_pretty(staged.as_file_mut(), registry)
            .map_err(|_| CredentialError::metadata_failed())?;
        staged
            .write_all(b"\n")
            .map_err(|_| CredentialError::metadata_failed())?;
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|_| CredentialError::metadata_failed())?;
        staged
            .persist(path)
            .map_err(|_| CredentialError::metadata_failed())?;
        Ok(())
    })();
    let unlock = lock
        .unlock()
        .map_err(|_| CredentialError::metadata_failed());
    match (result, unlock) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn validate_label(value: &str, maximum: usize) -> CredentialResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > maximum || trimmed.chars().any(char::is_control) {
        return Err(CredentialError::invalid());
    }
    Ok(())
}

fn validate_secret(value: &str, maximum: usize) -> CredentialResult<()> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(CredentialError::invalid());
    }
    Ok(())
}

fn validate_project_id(project_id: &str) -> CredentialResult<()> {
    if project_id.len() != 16
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(CredentialError::invalid());
    }
    Ok(())
}

fn validate_account_id(account_id: &str) -> CredentialResult<()> {
    if account_id.len() != 32
        || !account_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(CredentialError::invalid());
    }
    Ok(())
}

fn random_account_id() -> CredentialResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CredentialError::store_failed())?;
    let mut id = String::with_capacity(32);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        id.push(char::from(HEX[(byte >> 4) as usize]));
        id.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    Ok(id)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        values: Mutex<BTreeMap<String, Vec<u8>>>,
    }

    impl SecretStore for MemoryStore {
        fn put(&self, account_id: &str, secret: &[u8]) -> CredentialResult<()> {
            self.values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(account_id.to_owned(), secret.to_vec());
            Ok(())
        }

        fn get(&self, account_id: &str) -> CredentialResult<Zeroizing<Vec<u8>>> {
            self.values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(account_id)
                .cloned()
                .map(Zeroizing::new)
                .ok_or_else(CredentialError::secret_missing)
        }

        fn delete(&self, account_id: &str) -> CredentialResult<()> {
            self.values
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(account_id)
                .map(|_| ())
                .ok_or_else(CredentialError::secret_missing)
        }
    }

    fn vault(directory: &Path) -> CredentialVault {
        CredentialVault::new(
            directory.join("credentials.json"),
            Arc::new(MemoryStore::default()),
        )
    }

    fn input(project: Option<&str>) -> CreateAccountInput {
        CreateAccountInput {
            display_name: "Staging admin".to_owned(),
            service: "staging.example.test".to_owned(),
            username: Zeroizing::new("fake-user-canary".to_owned()),
            password: Zeroizing::new("fake-password-canary".to_owned()),
            grant_project_id: project.map(str::to_owned),
        }
    }

    #[test]
    fn metadata_never_contains_account_name_or_password() {
        let directory = tempfile::tempdir().expect("app data");
        let vault = vault(directory.path());
        let account = vault.create(input(None)).expect("create account");

        let bytes = fs::read(directory.path().join("credentials.json")).expect("metadata");
        assert!(
            !bytes
                .windows(b"fake-user-canary".len())
                .any(|item| item == b"fake-user-canary")
        );
        assert!(
            !bytes
                .windows(b"fake-password-canary".len())
                .any(|item| item == b"fake-password-canary")
        );
        assert!(
            bytes
                .windows(account.display_name.len())
                .any(|item| item == account.display_name.as_bytes())
        );
    }

    #[test]
    fn account_is_ungranted_until_desktop_grants_one_project() {
        let directory = tempfile::tempdir().expect("app data");
        let vault = vault(directory.path());
        let project = "0123456789abcdef";
        let account = vault.create(input(None)).expect("create account");

        assert!(!vault.list_for_project(project).expect("list")[0].allowed_for_project);
        assert_eq!(
            vault
                .secret_field(&account.id, project, AccountSecretField::Password)
                .expect_err("blocked")
                .code(),
            crate::CredentialErrorCode::ProjectNotAllowed
        );

        vault
            .set_project_access(&account.id, project, true)
            .expect("grant");
        let password = vault
            .secret_field(&account.id, project, AccountSecretField::Password)
            .expect("allowed");
        assert_eq!(password.as_str(), "fake-password-canary");
    }

    #[test]
    fn grants_are_independent_and_project_removal_keeps_the_account() {
        let directory = tempfile::tempdir().expect("app data");
        let vault = vault(directory.path());
        let first = "0123456789abcdef";
        let second = "fedcba9876543210";
        let account = vault.create(input(Some(first))).expect("create account");
        vault
            .set_project_access(&account.id, second, true)
            .expect("grant second");
        assert_eq!(
            vault.list_for_project(first).expect("list")[0].allowed_project_count,
            2
        );

        vault.revoke_project(first).expect("revoke project");
        assert!(!vault.list_for_project(first).expect("list")[0].allowed_for_project);
        assert!(vault.list_for_project(second).expect("list")[0].allowed_for_project);
    }

    #[test]
    fn updating_one_secret_field_preserves_the_other() {
        let directory = tempfile::tempdir().expect("app data");
        let vault = vault(directory.path());
        let project = "0123456789abcdef";
        let account = vault.create(input(Some(project))).expect("create account");
        vault
            .update(UpdateAccountInput {
                account_id: account.id.clone(),
                display_name: "Renamed".to_owned(),
                service: "staging.example.test".to_owned(),
                username: None,
                password: Some(Zeroizing::new("replacement-canary".to_owned())),
            })
            .expect("update");

        assert_eq!(
            vault
                .secret_field(&account.id, project, AccountSecretField::Username)
                .expect("username")
                .as_str(),
            "fake-user-canary"
        );
        assert_eq!(
            vault
                .secret_field(&account.id, project, AccountSecretField::Password)
                .expect("password")
                .as_str(),
            "replacement-canary"
        );
    }

    #[test]
    fn deleting_metadata_also_deletes_the_native_secret() {
        let directory = tempfile::tempdir().expect("app data");
        let store = Arc::new(MemoryStore::default());
        let vault = CredentialVault::new(directory.path().join("credentials.json"), store.clone());
        let account = vault.create(input(None)).expect("create account");

        vault.delete(&account.id).expect("delete");
        assert!(
            vault
                .list_for_project("0123456789abcdef")
                .expect("list")
                .is_empty()
        );
        assert!(
            !store
                .values
                .lock()
                .expect("store")
                .contains_key(&account.id)
        );
    }
}

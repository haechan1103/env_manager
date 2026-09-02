use zeroize::Zeroizing;

use crate::error::{CredentialError, CredentialResult};
use crate::service::SecretStore;

const NATIVE_SERVICE: &str = "dev.hgc.env-manager.credentials.v1";

pub struct NativeSecretStore;

impl NativeSecretStore {
    pub fn new() -> CredentialResult<Self> {
        keyring::Entry::store_status()
            .as_ref()
            .map_err(|_| CredentialError::store_unavailable())?;
        Ok(Self)
    }

    fn entry(account_id: &str) -> CredentialResult<keyring::Entry> {
        keyring::Entry::new(NATIVE_SERVICE, account_id)
            .map_err(|_| CredentialError::store_unavailable())
    }
}

impl SecretStore for NativeSecretStore {
    fn put(&self, account_id: &str, secret: &[u8]) -> CredentialResult<()> {
        Self::entry(account_id)?
            .set_secret(secret)
            .map_err(|_| CredentialError::store_failed())
    }

    fn get(&self, account_id: &str) -> CredentialResult<Zeroizing<Vec<u8>>> {
        Self::entry(account_id)?
            .get_secret()
            .map(Zeroizing::new)
            .map_err(|error| match error {
                keyring::Error::NoEntry => CredentialError::secret_missing(),
                _ => CredentialError::store_failed(),
            })
    }

    fn delete(&self, account_id: &str) -> CredentialResult<()> {
        Self::entry(account_id)?
            .delete_credential()
            .map_err(|error| match error {
                keyring::Error::NoEntry => CredentialError::secret_missing(),
                _ => CredentialError::store_failed(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "touches the current operating system credential store"]
    fn native_store_round_trip_uses_no_plaintext_fallback() {
        let store = NativeSecretStore::new().expect("native credential store");
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).expect("random account id");
        let account_id = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let fake_secret = b"fake-native-keyring-smoke-secret";

        let result = (|| {
            store.put(&account_id, fake_secret)?;
            let loaded = store.get(&account_id)?;
            if loaded.as_slice() != fake_secret {
                return Err(CredentialError::store_failed());
            }
            Ok(())
        })();
        let cleanup = store.delete(&account_id);

        result.expect("native round trip");
        cleanup.expect("native cleanup");
    }
}

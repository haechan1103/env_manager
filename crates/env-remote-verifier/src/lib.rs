use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use env_core::Document;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const PROTOCOL_VERSION: u32 = 1;
const MAX_CLOCK_SKEW_SECONDS: u64 = 90;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_ENV_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_IDENTITY_BYTES: u64 = 4096;
const MAX_COMPARISONS: usize = 100;
const REPLAY_RETENTION_SECONDS: u64 = 600;
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifierConfig {
    pub version: u32,
    pub identity_file: PathBuf,
    pub replay_directory: PathBuf,
    pub targets: BTreeMap<String, VerifierTarget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifierTarget {
    pub file: PathBuf,
    pub allowed_keys: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerifierState {
    Same,
    Different,
    Unset,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifierComparison {
    pub key: String,
    pub state: VerifierState,
    pub result_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifierResponse {
    pub protocol_version: u32,
    pub target_id: String,
    pub comparisons: Vec<VerifierComparison>,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifierError {
    #[error("{code}")]
    Stable { code: &'static str },
}

impl VerifierError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Stable { code } => code,
        }
    }
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SecretRequest {
    protocol_version: u32,
    request_id: String,
    issued_at_seconds: u64,
    target_id: String,
    comparisons: Vec<SecretComparison>,
}

impl Drop for SecretRequest {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SecretComparison {
    key: String,
    candidate: String,
}

pub fn encrypt_request<'a>(
    recipient: &str,
    target_id: &str,
    comparisons: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Vec<u8>, VerifierError> {
    validate_identifier(target_id, "REMOTE_TARGET_INVALID")?;
    let recipient = recipient
        .parse::<age::x25519::Recipient>()
        .map_err(|_| stable("REMOTE_RECIPIENT_INVALID"))?;
    let comparisons = comparisons
        .into_iter()
        .map(|(key, candidate)| SecretComparison {
            key: key.to_owned(),
            candidate: candidate.to_owned(),
        })
        .collect::<Vec<_>>();
    validate_comparisons(&comparisons)?;
    let request = SecretRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_request_id(),
        issued_at_seconds: now_seconds()?,
        target_id: target_id.to_owned(),
        comparisons,
    };
    let mut plaintext =
        serde_json::to_vec(&request).map_err(|_| stable("REMOTE_REQUEST_SERIALIZATION_FAILED"))?;
    let encrypted = age::encrypt(&recipient, &plaintext)
        .map_err(|_| stable("REMOTE_REQUEST_ENCRYPTION_FAILED"));
    plaintext.zeroize();
    encrypted
}

pub fn validate_recipient(recipient: &str) -> Result<(), VerifierError> {
    recipient
        .parse::<age::x25519::Recipient>()
        .map(|_| ())
        .map_err(|_| stable("REMOTE_RECIPIENT_INVALID"))
}

pub fn compare_encrypted_request(
    config: &VerifierConfig,
    encrypted: &[u8],
) -> Result<VerifierResponse, VerifierError> {
    validate_config(config)?;
    if encrypted.is_empty() || encrypted.len() > MAX_REQUEST_BYTES {
        return Err(stable("REMOTE_REQUEST_SIZE_INVALID"));
    }
    validate_identity_file(&config.identity_file)?;
    let identity_text = Zeroizing::new(
        fs::read_to_string(&config.identity_file)
            .map_err(|_| stable("REMOTE_IDENTITY_UNAVAILABLE"))?,
    );
    let identity = identity_text
        .trim()
        .parse::<age::x25519::Identity>()
        .map_err(|_| stable("REMOTE_IDENTITY_INVALID"))?;
    let mut plaintext = age::decrypt(&identity, encrypted)
        .map_err(|_| stable("REMOTE_REQUEST_DECRYPTION_FAILED"))?;
    if plaintext.len() > MAX_REQUEST_BYTES {
        plaintext.zeroize();
        return Err(stable("REMOTE_REQUEST_SIZE_INVALID"));
    }
    let request = serde_json::from_slice::<SecretRequest>(&plaintext)
        .map_err(|_| stable("REMOTE_REQUEST_INVALID"));
    plaintext.zeroize();
    let request = request?;
    validate_request(config, &request)?;
    claim_request_id(&config.replay_directory, &request.request_id)?;
    let target = config
        .targets
        .get(&request.target_id)
        .ok_or_else(|| stable("REMOTE_TARGET_UNKNOWN"))?;
    let document = load_target_document(&target.file)?;
    let mut results = Vec::with_capacity(request.comparisons.len());
    for comparison in &request.comparisons {
        if !target.allowed_keys.contains(&comparison.key) {
            results.push(VerifierComparison {
                key: comparison.key.clone(),
                state: VerifierState::Error,
                result_code: Some("REMOTE_KEY_NOT_ALLOWED".to_owned()),
            });
            continue;
        }
        let matches = document
            .assignments()
            .into_iter()
            .filter(|assignment| assignment.key == comparison.key)
            .count();
        if matches == 0 {
            results.push(VerifierComparison {
                key: comparison.key.clone(),
                state: VerifierState::Unset,
                result_code: None,
            });
            continue;
        }
        if matches != 1 {
            results.push(VerifierComparison {
                key: comparison.key.clone(),
                state: VerifierState::Error,
                result_code: Some("REMOTE_KEY_DUPLICATE".to_owned()),
            });
            continue;
        }
        let remote = Zeroizing::new(
            document
                .decoded_value(&comparison.key)
                .map_err(|_| stable("REMOTE_ENV_PARSE_FAILED"))?,
        );
        results.push(VerifierComparison {
            key: comparison.key.clone(),
            state: if constant_time_eq(remote.as_bytes(), comparison.candidate.as_bytes()) {
                VerifierState::Same
            } else {
                VerifierState::Different
            },
            result_code: None,
        });
    }
    Ok(VerifierResponse {
        protocol_version: PROTOCOL_VERSION,
        target_id: request.target_id.clone(),
        comparisons: results,
    })
}

pub fn load_config(path: &Path) -> Result<VerifierConfig, VerifierError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| stable("REMOTE_CONFIG_UNAVAILABLE"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_CONFIG_BYTES
    {
        return Err(stable("REMOTE_CONFIG_INVALID"));
    }
    let bytes = fs::read(path).map_err(|_| stable("REMOTE_CONFIG_UNAVAILABLE"))?;
    serde_json::from_slice(&bytes).map_err(|_| stable("REMOTE_CONFIG_INVALID"))
}

fn validate_identity_file(path: &Path) -> Result<(), VerifierError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| stable("REMOTE_IDENTITY_UNAVAILABLE"))?;
    if !path.is_absolute()
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_IDENTITY_BYTES
    {
        return Err(stable("REMOTE_IDENTITY_INVALID"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(stable("REMOTE_IDENTITY_PERMISSIONS_UNSAFE"));
        }
    }
    Ok(())
}

fn validate_config(config: &VerifierConfig) -> Result<(), VerifierError> {
    if config.version != 1 || config.targets.is_empty() {
        return Err(stable("REMOTE_CONFIG_INVALID"));
    }
    for (target_id, target) in &config.targets {
        validate_identifier(target_id, "REMOTE_CONFIG_INVALID")?;
        if !target.file.is_absolute() || target.allowed_keys.is_empty() {
            return Err(stable("REMOTE_CONFIG_INVALID"));
        }
        for key in &target.allowed_keys {
            validate_key(key)?;
        }
    }
    Ok(())
}

fn validate_request(config: &VerifierConfig, request: &SecretRequest) -> Result<(), VerifierError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(stable("REMOTE_PROTOCOL_UNSUPPORTED"));
    }
    validate_identifier(&request.request_id, "REMOTE_REQUEST_INVALID")?;
    validate_identifier(&request.target_id, "REMOTE_TARGET_INVALID")?;
    validate_comparisons(&request.comparisons)?;
    let now = now_seconds()?;
    if now.abs_diff(request.issued_at_seconds) > MAX_CLOCK_SKEW_SECONDS {
        return Err(stable("REMOTE_REQUEST_EXPIRED"));
    }
    if !config.targets.contains_key(&request.target_id) {
        return Err(stable("REMOTE_TARGET_UNKNOWN"));
    }
    Ok(())
}

fn validate_comparisons(comparisons: &[SecretComparison]) -> Result<(), VerifierError> {
    if comparisons.is_empty() || comparisons.len() > MAX_COMPARISONS {
        return Err(stable("REMOTE_COMPARISON_COUNT_INVALID"));
    }
    let mut unique = BTreeSet::new();
    for comparison in comparisons {
        validate_key(&comparison.key)?;
        if !unique.insert(comparison.key.as_str()) {
            return Err(stable("REMOTE_KEY_DUPLICATE"));
        }
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), VerifierError> {
    let mut characters = key.chars();
    let first = characters
        .next()
        .ok_or_else(|| stable("REMOTE_KEY_INVALID"))?;
    if !(first == '_' || first.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        || key.len() > 256
    {
        return Err(stable("REMOTE_KEY_INVALID"));
    }
    Ok(())
}

fn validate_identifier(value: &str, code: &'static str) -> Result<(), VerifierError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(stable(code));
    }
    Ok(())
}

fn load_target_document(path: &Path) -> Result<Document, VerifierError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| stable("REMOTE_FILE_UNAVAILABLE"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_ENV_FILE_BYTES
    {
        return Err(stable("REMOTE_FILE_INVALID"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| stable("REMOTE_FILE_UNAVAILABLE"))?;
    if canonical != path {
        return Err(stable("REMOTE_FILE_PATH_CHANGED"));
    }
    let bytes = fs::read(path).map_err(|_| stable("REMOTE_FILE_UNAVAILABLE"))?;
    Document::parse(bytes, path).map_err(|_| stable("REMOTE_ENV_PARSE_FAILED"))
}

fn claim_request_id(directory: &Path, request_id: &str) -> Result<(), VerifierError> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|_| stable("REMOTE_REPLAY_STORE_UNAVAILABLE"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(stable("REMOTE_REPLAY_STORE_INVALID"));
    }
    cleanup_replay_markers(directory);
    let path = directory.join(request_id);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| stable("REMOTE_REQUEST_REPLAYED"))?;
    Ok(())
}

fn cleanup_replay_markers(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten().take(1_000) {
        let path = entry.path();
        let expired = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age.as_secs() > REPLAY_RETENTION_SECONDS);
        if expired && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(path);
        }
    }
}

fn new_request_id() -> String {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("req-{nanos:x}-{:x}-{sequence:x}", std::process::id())
}

fn now_seconds() -> Result<u64, VerifierError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| stable("REMOTE_CLOCK_INVALID"))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

fn stable(code: &'static str) -> VerifierError {
    VerifierError::Stable { code }
}

pub fn read_bounded(mut reader: impl Read) -> Result<Vec<u8>, VerifierError> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| stable("REMOTE_REQUEST_READ_FAILED"))?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(stable("REMOTE_REQUEST_SIZE_INVALID"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, VerifierConfig, age::x25519::Recipient) {
        let directory = tempfile::tempdir().expect("fixture");
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let identity_path = directory.path().join("identity.agekey");
        fs::write(&identity_path, identity.to_string().expose_secret()).expect("identity fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&identity_path, fs::Permissions::from_mode(0o600))
                .expect("identity permissions");
        }
        let replay_directory = directory.path().join("replay");
        fs::create_dir(&replay_directory).expect("replay fixture");
        let file = directory.path().join("runtime.env");
        fs::write(
            &file,
            "DEMO_TOKEN=fake_remote_canary\nEMPTY=\nDUP=one\nDUP=two\n",
        )
        .expect("env fixture");
        let file = file.canonicalize().expect("canonical env fixture");
        let config = VerifierConfig {
            version: 1,
            identity_file: identity_path,
            replay_directory,
            targets: BTreeMap::from([(
                "mobile-ok-dev".to_owned(),
                VerifierTarget {
                    file,
                    allowed_keys: BTreeSet::from([
                        "DEMO_TOKEN".to_owned(),
                        "MISSING".to_owned(),
                        "DUP".to_owned(),
                    ]),
                },
            )]),
        };
        (directory, config, recipient)
    }

    use age::secrecy::ExposeSecret;

    #[test]
    fn encrypted_compare_returns_only_redacted_states() {
        let (_directory, config, recipient) = fixture();
        let encrypted = encrypt_request(
            &recipient.to_string(),
            "mobile-ok-dev",
            [
                ("DEMO_TOKEN", "fake_remote_canary"),
                ("MISSING", "fake_missing"),
                ("DUP", "fake_duplicate"),
            ],
        )
        .expect("encrypt request");
        let response = compare_encrypted_request(&config, &encrypted).expect("compare request");
        assert_eq!(response.comparisons[0].state, VerifierState::Same);
        assert_eq!(response.comparisons[1].state, VerifierState::Unset);
        assert_eq!(response.comparisons[2].state, VerifierState::Error);
        let serialized = serde_json::to_string(&response).expect("serialize response");
        assert!(!serialized.contains("fake_remote_canary"));
        assert!(!serialized.contains("fake_missing"));
        assert!(!serialized.contains("fake_duplicate"));
    }

    #[test]
    fn replayed_ciphertext_is_rejected() {
        let (_directory, config, recipient) = fixture();
        let encrypted = encrypt_request(
            &recipient.to_string(),
            "mobile-ok-dev",
            [("DEMO_TOKEN", "fake_remote_canary")],
        )
        .expect("encrypt request");
        compare_encrypted_request(&config, &encrypted).expect("first request");
        let error = compare_encrypted_request(&config, &encrypted).expect_err("replay rejected");
        assert_eq!(error.code(), "REMOTE_REQUEST_REPLAYED");
    }

    #[test]
    fn unlisted_key_is_redacted_and_denied() {
        let (_directory, config, recipient) = fixture();
        let encrypted = encrypt_request(
            &recipient.to_string(),
            "mobile-ok-dev",
            [("OTHER_SECRET", "fake_other_secret")],
        )
        .expect("encrypt request");
        let response = compare_encrypted_request(&config, &encrypted).expect("compare request");
        assert_eq!(response.comparisons[0].state, VerifierState::Error);
        assert_eq!(
            response.comparisons[0].result_code.as_deref(),
            Some("REMOTE_KEY_NOT_ALLOWED")
        );
    }
}

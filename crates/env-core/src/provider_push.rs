use zeroize::Zeroizing;

/// One value selected for a user-initiated provider push.
///
/// The type intentionally has no `Debug`, `Display`, or serialization implementation.
/// It may cross only the in-process core-to-Tauri boundary before being written to a
/// provider CLI's standard input.
pub struct ProviderValue {
    key: String,
    value: Zeroizing<String>,
}

impl ProviderValue {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self {
            key,
            value: Zeroizing::new(value),
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        self.value.as_str()
    }
}

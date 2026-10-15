//! Secure storage for secrets (AI API keys).
//!
//! Uses the OS credential vault (Windows Credential Manager via the `keyring`
//! crate). Secrets never touch SQLite, logs, or the frontend state — only a
//! boolean "configured" flag is exposed to the UI.

const SERVICE: &str = "ClipDeck";
const KEY_OPENROUTER_API: &str = "openrouter_api_key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, KEY_OPENROUTER_API).map_err(|e| format!("keyring error: {e}"))
}

/// Store the OpenRouter API key in the OS credential vault.
pub fn store_openrouter_api_key(key: &str) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return clear_openrouter_api_key();
    }
    entry()?
        .set_password(trimmed)
        .map_err(|e| format!("failed to store key: {e}"))
}

/// Read the OpenRouter API key (never log this value).
pub fn get_openrouter_api_key() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|s| !s.is_empty())
}

/// Whether an API key is configured (no secret exposure).
pub fn has_openrouter_api_key() -> bool {
    get_openrouter_api_key().is_some()
}

/// Remove the stored key.
pub fn clear_openrouter_api_key() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("failed to delete key: {e}")),
    }
}

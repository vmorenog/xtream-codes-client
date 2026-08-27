//! **Provider** passwords, in the macOS Keychain.
//!
//! The database stores a Provider's name, base URL and username. The password
//! never touches SQLite, never reaches the webview, and never appears in a log.

use keyring::Entry;

use crate::error::Result;

const SERVICE: &str = "com.vmorenog.xtream-codes-client";

fn entry(provider_id: i64) -> Result<Entry> {
    Ok(Entry::new(SERVICE, &format!("provider-{provider_id}"))?)
}

pub fn store(provider_id: i64, password: &str) -> Result<()> {
    entry(provider_id)?.set_password(password)?;
    Ok(())
}

pub fn load(provider_id: i64) -> Result<String> {
    Ok(entry(provider_id)?.get_password()?)
}

/// Best-effort: a Provider with no stored password is already in the state we
/// want, so a missing entry is not an error.
pub fn forget(provider_id: i64) {
    if let Ok(e) = entry(provider_id) {
        let _ = e.delete_credential();
    }
}

//! Session tokens in the OS keyring (Secret Service on Linux, Credential
//! Manager on Windows). Without a keyring the token is kept in memory only,
//! so the user logs in again on the next launch.

use beholder_protocol::SessionToken;
use keyring_core::Entry;

const SERVICE: &str = "beholder";

/// Access to the OS keyring, if one is available.
pub struct Secrets {
    available: bool,
}

impl Secrets {
    /// Connects to the platform keyring.
    pub fn init() -> Self {
        Self {
            available: install_store(),
        }
    }

    fn entry(&self, server_url: &str) -> Option<Entry> {
        if !self.available {
            return None;
        }
        Entry::new(SERVICE, server_url).ok()
    }

    /// The stored token for a server.
    pub fn load(&self, server_url: &str) -> Option<SessionToken> {
        let secret = self.entry(server_url)?.get_password().ok()?;
        SessionToken::from_hex(&secret)
    }

    /// Stores the token for a server; `false` if it couldn't be persisted.
    pub fn save(&self, server_url: &str, token: &SessionToken) -> bool {
        self.entry(server_url)
            .is_some_and(|e| e.set_password(&token.to_hex()).is_ok())
    }

    /// Forgets the token for a server.
    pub fn delete(&self, server_url: &str) {
        if let Some(e) = self.entry(server_url) {
            let _ = e.delete_credential();
        }
    }
}

#[cfg(target_os = "linux")]
fn install_store() -> bool {
    match dbus_secret_service_keyring_store::Store::new() {
        Ok(store) => {
            keyring_core::set_default_store(store);
            true
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn install_store() -> bool {
    match windows_native_keyring_store::Store::new() {
        Ok(store) => {
            keyring_core::set_default_store(store);
            true
        }
        Err(_) => false,
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn install_store() -> bool {
    false
}

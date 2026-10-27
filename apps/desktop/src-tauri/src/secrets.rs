use beholder_protocol::SessionToken;
use keyring_core::Entry;

const SERVICE: &str = "beholder";

pub struct Secrets {
    available: bool,
}

impl Secrets {
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

    pub fn load(&self, server_url: &str) -> Option<SessionToken> {
        let secret = self.entry(server_url)?.get_password().ok()?;
        SessionToken::from_hex(&secret)
    }

    pub fn save(&self, server_url: &str, token: &SessionToken) -> bool {
        self.entry(server_url)
            .is_some_and(|e| e.set_password(&token.to_hex()).is_ok())
    }

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

//! Local storage for provider API keys (currently just DeepSeek) so the
//! installed app doesn't require launching from a shell with the key
//! exported. Keys never touch the SQLite config table or the webview —
//! only this module's callers (`generator::run_deepseek`) read them, and
//! only a configured/not-configured boolean is ever sent to the frontend.
//!
//! macOS-only: backed by the login Keychain via the `security` CLI. On
//! other platforms, only the documented environment-variable path applies
//! (see README) — `set_secret` reports that explicitly rather than
//! silently discarding the key.

const SERVICE: &str = "system-design-roulette";

fn account_for(name: &str) -> String {
    format!("{name}_api_key")
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{account_for, SERVICE};
    use std::process::Command;

    pub fn get_secret(name: &str) -> Option<String> {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-a",
                &account_for(name),
                "-s",
                SERVICE,
                "-w",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    pub fn set_secret(name: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() {
            return delete_secret(name);
        }
        // `-U` updates the item in place if it already exists, so re-saving
        // a key doesn't create duplicate keychain entries.
        let status = Command::new("security")
            .args([
                "add-generic-password",
                "-a",
                &account_for(name),
                "-s",
                SERVICE,
                "-w",
                value,
                "-U",
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("keychain write failed".into())
        }
    }

    pub fn delete_secret(name: &str) -> Result<(), String> {
        let status = Command::new("security")
            .args([
                "delete-generic-password",
                "-a",
                &account_for(name),
                "-s",
                SERVICE,
            ])
            .status()
            .map_err(|e| e.to_string())?;
        // Exit code 44 is "item not found" — clearing an already-clear key
        // is not an error.
        if status.success() || status.code() == Some(44) {
            Ok(())
        } else {
            Err("keychain delete failed".into())
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn get_secret(_name: &str) -> Option<String> {
        None
    }

    pub fn set_secret(_name: &str, _value: &str) -> Result<(), String> {
        Err(
            "in-app key storage is only available on macOS — export the \
             environment variable instead (see README)"
                .into(),
        )
    }

    pub fn delete_secret(_name: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Read a stored secret. `None` covers "never set", "keychain locked", and
/// "`security` unavailable" alike — callers treat all three as not configured.
pub fn get_secret(name: &str) -> Option<String> {
    imp::get_secret(name)
}

/// Store (or clear, if `value` is blank) a secret.
pub fn set_secret(name: &str, value: &str) -> Result<(), String> {
    imp::set_secret(name, value)
}

pub fn has_secret(name: &str) -> bool {
    get_secret(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_naming_is_namespaced_per_provider() {
        assert_eq!(account_for("deepseek"), "deepseek_api_key");
        assert_ne!(account_for("deepseek"), account_for("openai"));
    }
}

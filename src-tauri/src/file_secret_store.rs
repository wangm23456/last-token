use std::collections::HashMap;
use std::path::PathBuf;
use parking_lot::Mutex;
use crate::storage::SecretStore;

/// File-backed secret store that persists secrets as a JSON file in the
/// app data directory. Read-through from file on first access, write-through
/// on set/delete. No encryption — secrets are stored as plaintext JSON.
///
/// Windows skips the slow shell-env fallback entirely (cmd spawn is expensive
/// and provides no value over process env). POSIX keeps the shell fallback
/// for GUI-launched apps that inherit a minimal environment.
pub struct FileSecretStore {
    path: PathBuf,
    cache: Mutex<HashMap<String, String>>,
    loaded: Mutex<bool>,
}

impl FileSecretStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let path = app_data_dir.join("secrets.json");
        Self {
            path,
            cache: Mutex::new(HashMap::new()),
            loaded: Mutex::new(false),
        }
    }

    fn ensure_loaded(&self) {
        let mut loaded = self.loaded.lock();
        if *loaded {
            return;
        }
        *loaded = true;
        drop(loaded);

        if let Ok(content) = std::fs::read_to_string(&self.path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                let mut cache = self.cache.lock();
                *cache = map;
            }
        }
    }

    fn persist(&self) {
        let cache = self.cache.lock();
        if let Ok(json) = serde_json::to_string_pretty(&*cache) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

impl SecretStore for FileSecretStore {
    fn get_secret(&self, account_id: &str, secret_name: &str) -> Result<Option<String>, String> {
        self.ensure_loaded();

        // Fast path: process environment (works when launched from a terminal).
        let provider = account_id.split(':').next().unwrap_or("");
        let candidates = env_var_candidates(provider, secret_name);
        for name in &candidates {
            if let Ok(value) = std::env::var(name) {
                if !value.is_empty() {
                    return Ok(Some(value));
                }
            }
        }

        // Fallback: file-backed store.
        let key = format!("account/{}/{}", account_id, secret_name);
        if let Some(value) = self.cache.lock().get(&key) {
            return Ok(Some(value.clone()));
        }

        // POSIX fallback: read from the user's login shell environment
        // (needed when launched from the GUI on macOS, where the app
        // inherits a minimal env). Windows skips this entirely.
        #[cfg(not(target_os = "windows"))]
        for name in candidates {
            if let Some(value) = shell_env_value(name) {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    fn set_secret(&self, account_id: &str, secret_name: &str, value: &str) -> Result<(), String> {
        self.ensure_loaded();
        let key = format!("account/{}/{}", account_id, secret_name);
        self.cache.lock().insert(key, value.to_string());
        self.persist();
        Ok(())
    }

    fn delete_secret(&self, account_id: &str, secret_name: &str) -> Result<(), String> {
        self.ensure_loaded();
        let key = format!("account/{}/{}", account_id, secret_name);
        self.cache.lock().remove(&key);
        self.persist();
        Ok(())
    }
}

/// Predefined environment variable names for each provider/secret combination.
fn env_var_candidates(provider: &str, secret_name: &str) -> Vec<&'static str> {
    match (provider, secret_name) {
        ("claude", "api_key") => vec!["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "CLAUDE_CODE_API_KEY", "ANTHROPIC_AUTH_TOKEN"],
        ("codex", "api_key") => vec!["OPENAI_API_KEY", "CODEX_API_KEY"],
        ("gemini", "api_key") => vec!["GOOGLE_API_KEY", "GEMINI_API_KEY", "GOOGLE_GENERATIVE_AI_API_KEY", "GEMINI_CLI_API_KEY"],
        ("kimi", "api_key") => vec!["MOONSHOT_API_KEY", "KIMI_API_KEY", "KIMI_CODING_API_KEY", "KIMI_KEY", "MOONSHOT_KEY"],
        ("zhipu", "api_key") | ("zhipu_team", "api_key") => vec!["ZHIPU_API_KEY", "GLM_API_KEY", "ZHIPUAI_API_KEY", "BIGMODEL_API_KEY"],
        ("minimax", "api_key") => vec!["MINIMAX_API_KEY", "MINIMAX_CODING_API_KEY", "MINIMAX_GROUP_ID", "MINIMAX_API_SECRET"],
        ("zenmux", "api_key") => vec!["ZENMUX_API_KEY", "ZENMUX_KEY"],
        ("copilot", "github_token") => vec!["GITHUB_TOKEN", "GH_TOKEN", "GITHUB_COPILOT_TOKEN", "COPILOT_TOKEN"],
        ("volcengine", "access_key_id") => vec!["VOLCENGINE_ACCESS_KEY_ID", "VOLCENGINE_ACCESS_KEY", "VOLC_ACCESS_KEY", "ARK_ACCESS_KEY"],
        ("volcengine", "secret_access_key") => vec!["VOLCENGINE_SECRET_ACCESS_KEY", "VOLCENGINE_SECRET_KEY", "VOLC_SECRET_KEY", "ARK_SECRET_KEY"],
        _ => vec![],
    }
}

/// Spawn a login shell to read the value of a single environment variable.
/// This captures the user's shell environment rather than the Tauri process's
/// environment, which is usually empty when launched from the GUI on macOS.
#[cfg(not(target_os = "windows"))]
fn shell_env_value(name: &str) -> Option<String> {
    let output = shell_command_for_env(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(not(target_os = "windows"))]
fn shell_command_for_env(name: &str) -> std::process::Command {
    let user_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = std::process::Command::new(user_shell);
    // Execute interactive login shell (-l -i -c) to ensure .zshrc / .zprofile / .bashrc are loaded
    cmd.args(["-l", "-i", "-c"]);
    cmd.arg(format!("echo ${}", name));
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_secret_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().to_path_buf());
        let account = "test:account";

        assert!(store.get_secret(account, "x").unwrap().is_none());

        store.set_secret(account, "x", "cached-value").unwrap();
        assert_eq!(store.get_secret(account, "x").unwrap(), Some("cached-value".to_string()));

        // Reload from file.
        let store2 = FileSecretStore::new(dir.path().to_path_buf());
        assert_eq!(store2.get_secret(account, "x").unwrap(), Some("cached-value".to_string()));

        store2.delete_secret(account, "x").unwrap();
        assert!(store2.get_secret(account, "x").unwrap().is_none());
    }
}

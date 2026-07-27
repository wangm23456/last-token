use std::collections::HashMap;
use parking_lot::Mutex;
use crate::storage::SecretStore;

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

#[cfg(target_os = "windows")]
fn shell_command_for_env(name: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/c"]);
    cmd.arg(format!("echo %{}%", name));
    cmd
}

/// Credential store that reads secrets from a predefined set of environment
/// variables via the user's shell. Set/delete are backed by an in-memory cache
/// so runtime secrets (e.g. OAuth tokens from device flow) still work without
/// persisting to the OS keychain.
pub struct ShellEnvSecretStore {
    cache: Mutex<HashMap<String, String>>,
}

impl ShellEnvSecretStore {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl SecretStore for ShellEnvSecretStore {
    fn get_secret(&self, account_id: &str, secret_name: &str) -> Result<Option<String>, String> {
        let provider = account_id.split(':').next().unwrap_or("");
        let candidates = env_var_candidates(provider, secret_name);

        // Fast path: process environment (works when launched from a terminal).
        for name in &candidates {
            if let Ok(value) = std::env::var(name) {
                if !value.is_empty() {
                    return Ok(Some(value));
                }
            }
        }

        // Fallback: read from the user's login shell environment (needed when
        // launched from the GUI on macOS, where the app inherits a minimal env).
        for name in candidates {
            if let Some(value) = shell_env_value(name) {
                return Ok(Some(value));
            }
        }

        // Final fallback: in-memory cache (e.g. OAuth tokens set during device flow).
        let key = format!("account/{}/{}", account_id, secret_name);
        Ok(self.cache.lock().get(&key).cloned())
    }

    fn set_secret(&self, account_id: &str, secret_name: &str, value: &str) -> Result<(), String> {
        let key = format!("account/{}/{}", account_id, secret_name);
        self.cache.lock().insert(key, value.to_string());
        Ok(())
    }

    fn delete_secret(&self, account_id: &str, secret_name: &str) -> Result<(), String> {
        let key = format!("account/{}/{}", account_id, secret_name);
        self.cache.lock().remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_env_store_memory_cache() {
        let store = ShellEnvSecretStore::new();
        let account = "custom:no-env-match";

        assert!(store.get_secret(account, "x").unwrap().is_none());

        store.set_secret(account, "x", "cached-value").unwrap();
        assert_eq!(store.get_secret(account, "x").unwrap(), Some("cached-value".to_string()));

        store.delete_secret(account, "x").unwrap();
        assert!(store.get_secret(account, "x").unwrap().is_none());
    }
}

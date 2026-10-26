//! Minimal i18n: dotted-key lookup into the bundled locale JSON files with
//! i18next-style `{{name}}` interpolation. No runtime dependency.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde_json::Value;

pub const SUPPORTED_LANGUAGES: [&str; 2] = ["zh-CN", "en"];
pub const DEFAULT_LANGUAGE: &str = "zh-CN";

static BUNDLES: LazyLock<HashMap<&'static str, Value>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "en",
        serde_json::from_str::<Value>(include_str!("../assets/i18n/en.json"))
            .expect("en.json is valid JSON"),
    );
    map.insert(
        "zh-CN",
        serde_json::from_str::<Value>(include_str!("../assets/i18n/zh-CN.json"))
            .expect("zh-CN.json is valid JSON"),
    );
    map
});

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    let mut node = root;
    for part in key.split('.') {
        node = node.get(part)?;
    }
    node.as_str()
}

fn interpolate(template: &str, params: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in params {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

/// Translate `key` for `lang`, falling back to the default language and then
/// to the key itself so a missing entry is visible rather than silent.
pub fn tr(lang: &str, key: &str) -> String {
    tr_params(lang, key, &[])
}

pub fn tr_params(lang: &str, key: &str, params: &[(&str, &str)]) -> String {
    let bundles = &BUNDLES;
    let template = bundles
        .get(lang)
        .and_then(|b| lookup(b, key))
        .or_else(|| bundles.get(DEFAULT_LANGUAGE).and_then(|b| lookup(b, key)));
    match template {
        Some(t) => interpolate(t, params),
        None => key.to_string(),
    }
}

// ── Language persistence ─────────────────────────────────────────
// A one-line file next to the SQLite database; the settings table predates
// localization and keeps its shape.

fn lang_file() -> std::path::PathBuf {
    crate::service::app_data_dir().join("last-token.lang")
}

pub fn detect_language() -> String {
    if let Ok(saved) = std::fs::read_to_string(lang_file()) {
        let saved = saved.trim().to_string();
        if SUPPORTED_LANGUAGES.contains(&saved.as_str()) {
            return saved;
        }
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(key) {
            if val.to_lowercase().starts_with("zh") {
                return "zh-CN".to_string();
            }
            if val.to_lowercase().starts_with("en") {
                return "en".to_string();
            }
        }
    }
    DEFAULT_LANGUAGE.to_string()
}

pub fn save_language(lang: &str) -> std::io::Result<()> {
    std::fs::write(lang_file(), lang)
}

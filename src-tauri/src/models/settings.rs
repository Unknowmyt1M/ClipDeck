use serde::{Deserialize, Serialize};

/// Application settings (key/value in SQLite, typed here).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // General
    pub theme: String, // "system" | "light" | "dark"
    pub startup: bool,
    pub global_hotkey: String,
    pub history_limit: i64,
    pub close_to_tray: bool,
    /// After picking a clip in the quick window, focus the previously
    /// active app and send Ctrl+V (the paste ClipboardManager users expect).
    pub auto_paste: bool,

    // Privacy
    pub pause_monitoring: bool,
    pub sensitive_detection: bool,
    /// Never save items detected as sensitive (when true).
    pub skip_sensitive: bool,
    /// JSON array of excluded application executables (e.g. "keepass.exe").
    pub excluded_apps: String,
    /// JSON array of excluded window-title substrings.
    pub excluded_titles: String,
    /// Retention: days to keep items (0 = keep forever).
    pub retention_days: i64,

    // AI
    pub ai_enabled: bool,
    pub ai_provider: String, // "openrouter"
    pub ai_model: String,
    pub ai_model_name: String,
    pub ai_free: bool,
    pub ai_auto_analyze: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            startup: false,
            global_hotkey: "Ctrl+Shift+V".into(),
            history_limit: 500,
            close_to_tray: true,
            auto_paste: true,
            pause_monitoring: false,
            sensitive_detection: true,
            skip_sensitive: false,
            excluded_apps: "[]".into(),
            excluded_titles: "[]".into(),
            retention_days: 0,
            ai_enabled: false,
            ai_provider: "openrouter".into(),
            ai_model: String::new(),
            ai_model_name: String::new(),
            ai_free: true,
            ai_auto_analyze: false,
        }
    }
}

impl Settings {
    /// Parse the excluded-apps list.
    pub fn excluded_apps_list(&self) -> Vec<String> {
        parse_json_list(&self.excluded_apps)
    }

    /// Parse the excluded-titles list.
    pub fn excluded_titles_list(&self) -> Vec<String> {
        parse_json_list(&self.excluded_titles)
    }
}

fn parse_json_list(s: &str) -> Vec<String> {
    let entries: Vec<String> = serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| {
        // Tolerate a legacy plain value (comma/newline separated).
        s.split([',', '\n']).map(str::to_string).collect()
    });
    entries
        .iter()
        .map(|v| {
            // Older UI versions double-encoded entries through the textarea
            // (e.g. `["[\"keepass.exe\"]"]`); strip leftover JSON delimiters
            // so the value still matches executable names/titles.
            v.trim()
                .trim_matches(['[', ']', '"', '\''])
                .trim()
                .to_lowercase()
        })
        .filter(|v| !v.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_array() {
        let s = Settings {
            excluded_apps: r#"["keepass.exe","1Password.exe"]"#.into(),
            ..Settings::default()
        };
        assert_eq!(s.excluded_apps_list(), vec!["keepass.exe", "1password.exe"]);
    }

    #[test]
    fn heals_double_encoded_entries() {
        // Exactly what old SettingsPage.saveList produced from
        // `["keepass.exe"]` and `["a.exe","b.exe"]` respectively.
        let one = Settings {
            excluded_apps: r#"["[\"keepass.exe\"]"]"#.into(),
            ..Settings::default()
        };
        assert_eq!(one.excluded_apps_list(), vec!["keepass.exe"]);

        let two = Settings {
            excluded_titles: r#"["[\"a.exe\"", "\"b.exe\"]"]"#.into(),
            ..Settings::default()
        };
        assert_eq!(two.excluded_titles_list(), vec!["a.exe", "b.exe"]);
    }

    #[test]
    fn falls_back_to_plain_text_values() {
        let s = Settings {
            excluded_apps: "keepass.exe, 1password.exe".into(),
            ..Settings::default()
        };
        assert_eq!(s.excluded_apps_list(), vec!["keepass.exe", "1password.exe"]);
    }
}

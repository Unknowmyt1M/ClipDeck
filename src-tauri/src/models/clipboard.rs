use serde::{Deserialize, Serialize};

/// Content type of a clipboard item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardItemType {
    Text,
    Url,
    Image,
}

impl ClipboardItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClipboardItemType::Text => "text",
            ClipboardItemType::Url => "url",
            ClipboardItemType::Image => "image",
        }
    }

    /// Parse a stored type string into an enum variant.
    /// Deliberately not named `from_str` to avoid clippy's
    /// `should_implement_trait` false-positive for our own semantic.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "url" => ClipboardItemType::Url,
            "image" => ClipboardItemType::Image,
            _ => ClipboardItemType::Text,
        }
    }
}

/// A single clipboard history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub uuid: String,
    #[serde(rename = "type")]
    pub item_type: ClipboardItemType,
    pub content: String,
    pub content_hash: String,
    pub preview: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    pub use_count: i64,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_sensitive: bool,
    /// AI summary (attached on list reads; `None` until analyzed).
    #[serde(default)]
    pub ai_summary: Option<String>,
    /// AI category label (attached on list reads).
    #[serde(default)]
    pub ai_category: Option<String>,
    /// AI keywords (attached on list reads).
    #[serde(default)]
    pub ai_keywords: Vec<String>,
    /// Linked tag names (manual + AI).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Classify plain text content into a clipboard item type.
///
/// Text that looks like a web URL (e.g. `https://example.com`) is classified
/// as `Url`; everything else is `Text`.
pub fn detect_type(text: &str) -> ClipboardItemType {
    let trimmed = text.trim();
    if trimmed.len() <= 2048 && looks_like_url(trimmed) {
        ClipboardItemType::Url
    } else {
        ClipboardItemType::Text
    }
}

fn looks_like_url(s: &str) -> bool {
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else if let Some(r) = lower.strip_prefix("www.") {
        r
    } else {
        return false;
    };
    !rest.is_empty() && rest.contains('.') && !rest.starts_with('.')
}

/// Build a short, single-line preview for list rendering.
pub fn make_preview(text: &str) -> String {
    const MAX_LEN: usize = 200;
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > MAX_LEN {
        let truncated: String = collapsed.chars().take(MAX_LEN).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_https_url() {
        assert_eq!(
            detect_type("https://example.com/path?q=1"),
            ClipboardItemType::Url
        );
    }

    #[test]
    fn detects_www_url() {
        assert_eq!(detect_type("www.example.com"), ClipboardItemType::Url);
    }

    #[test]
    fn plain_text_is_text() {
        assert_eq!(detect_type("hello world"), ClipboardItemType::Text);
        assert_eq!(detect_type("just-a-word"), ClipboardItemType::Text);
    }

    #[test]
    fn url_with_spaces_is_text() {
        assert_eq!(
            detect_type("https://example.com not a url"),
            ClipboardItemType::Text
        );
    }

    #[test]
    fn preview_collapses_whitespace() {
        assert_eq!(make_preview("a\n\n  b\t c"), "a b c");
    }

    #[test]
    fn preview_truncates_long_text() {
        let long = "x".repeat(300);
        let preview = make_preview(&long);
        assert_eq!(preview.chars().count(), 201); // 200 + ellipsis
        assert!(preview.ends_with('…'));
    }
}

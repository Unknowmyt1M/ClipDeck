//! Local sensitive-content detection.
//!
//! Runs entirely on-device. Detected content is flagged `is_sensitive` and is
//! **never sent to an external AI provider automatically** (enforced in the
//! AI queue) and never logged.

use regex::Regex;
use std::sync::OnceLock;

/// Result of scanning clipboard text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensitiveKind {
    /// 4-8 digit codes adjacent to otp/code/verification keywords.
    Otp,
    /// Payment card-shaped digit runs (13-19 digits with separators).
    CardNumber,
    /// Private key blocks.
    PrivateKey,
    /// Known token/key shapes (AWS, GitHub, OpenAI, Slack, JWT, Google…).
    ApiKey,
    /// key=value or key: value where the key names a secret.
    SecretAssignment,
    /// Bearer/Basic authorization headers.
    AuthHeader,
}

impl SensitiveKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SensitiveKind::Otp => "otp",
            SensitiveKind::CardNumber => "card_number",
            SensitiveKind::PrivateKey => "private_key",
            SensitiveKind::ApiKey => "api_key",
            SensitiveKind::SecretAssignment => "secret_assignment",
            SensitiveKind::AuthHeader => "auth_header",
        }
    }
}

struct Patterns {
    otp: Regex,
    card: Regex,
    private_key: Regex,
    aws_key: Regex,
    github_token: Regex,
    openai_key: Regex,
    slack_token: Regex,
    google_key: Regex,
    jwt: Regex,
    generic_token: Regex,
    secret_assignment: Regex,
    auth_header: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // "your OTP is 483921" / "otp: 123456" / "verification code 998877"
        otp: Regex::new(
            r"(?i)\b(otp|one[-\s]?time(?:\s+password)?|verification\s+code|auth(?:entication)?\s+code)\b[^0-9]{0,30}\b(\d{4,8})\b",
        )
        .unwrap(),
        // 13-19 digits with optional space/dash separators, Luhn-checked later.
        card: Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").unwrap(),
        private_key: Regex::new(
            r"-----BEGIN\s+(?:RSA\s+|EC\s+|DSA\s+|OPENSSH\s+|PGP\s+)?PRIVATE KEY(?:\s+BLOCK)?-----",
        )
        .unwrap(),
        aws_key: Regex::new(r"\b(?:AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16}\b").unwrap(),
        github_token: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{20,255}\b").unwrap(),
        openai_key: Regex::new(r"\bsk-(?:proj-|rat-)?[A-Za-z0-9_\-]{20,}\b").unwrap(),
        slack_token: Regex::new(r"\bxox[baprs]-[0-9A-Za-z-]{10,}\b").unwrap(),
        google_key: Regex::new(r"\bAIza[0-9A-Za-z_\-]{35}\b").unwrap(),
        jwt: Regex::new(r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{5,}\b")
            .unwrap(),
        generic_token: Regex::new(
            r#"(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|secret[_-]?key)\b\s*[:=]\s*["']?[A-Za-z0-9_\-.]{16,}"#,
        )
        .unwrap(),
        secret_assignment: Regex::new(
            r#"(?i)\b(password|passwd|pwd|secret|token|passphrase)\b\s*[:=]\s*["']?[^\s"']{4,}"#,
        )
        .unwrap(),
        auth_header: Regex::new(r"(?i)\b(authorization)\s*:\s*(bearer|basic)\s+[A-Za-z0-9_\-\.+/=]{10,}").unwrap(),
    })
}

/// Returns the first sensitive kind detected in `text`, if any.
///
/// Purely local; never logs or transmits the content.
pub fn detect(text: &str) -> Option<SensitiveKind> {
    let p = patterns();

    if p.otp.is_match(text) {
        return Some(SensitiveKind::Otp);
    }
    if p.private_key.is_match(text) || p.jwt.is_match(text) {
        return Some(SensitiveKind::PrivateKey);
    }
    if p.aws_key.is_match(text)
        || p.github_token.is_match(text)
        || p.openai_key.is_match(text)
        || p.slack_token.is_match(text)
        || p.google_key.is_match(text)
        || p.generic_token.is_match(text)
    {
        return Some(SensitiveKind::ApiKey);
    }
    if p.auth_header.is_match(text) {
        return Some(SensitiveKind::AuthHeader);
    }
    if p.card.contains_luhn_match(text) {
        return Some(SensitiveKind::CardNumber);
    }
    if p.secret_assignment.is_match(text) {
        return Some(SensitiveKind::SecretAssignment);
    }
    None
}

/// True if any sensitive content was detected (convenience for callers).
pub fn is_sensitive(text: &str) -> bool {
    detect(text).is_some()
}

/// Luhn helper extension so card candidates reduce false positives.
trait LuhnCheck {
    fn contains_luhn_match(&self, text: &str) -> bool;
}

impl LuhnCheck for Regex {
    fn contains_luhn_match(&self, text: &str) -> bool {
        for m in self.find_iter(text) {
            let digits: Vec<u8> = m
                .as_str()
                .bytes()
                .filter(|b| b.is_ascii_digit())
                .map(|b| b - b'0')
                .collect();
            if (13..=19).contains(&digits.len()) && luhn_valid(&digits) {
                return true;
            }
        }
        false
    }
}

fn luhn_valid(digits: &[u8]) -> bool {
    let mut sum = 0u32;
    for (i, &d) in digits.iter().rev().enumerate() {
        let mut d = d as u32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_otp() {
        let kind = detect("Your OTP is 483921").expect("should detect");
        assert_eq!(kind, SensitiveKind::Otp);
    }

    #[test]
    fn detects_card() {
        // 4111 1111 1111 1111 (Visa test number, Luhn valid)
        assert_eq!(
            detect("card: 4111 1111 1111 1111"),
            Some(SensitiveKind::CardNumber)
        );
    }

    #[test]
    fn rejects_non_card_digits() {
        // Luhn fails → not a card
        assert_eq!(detect("order number 1234567890123"), None);
    }

    #[test]
    fn detects_private_key() {
        let text = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQ...\n";
        assert_eq!(detect(text), Some(SensitiveKind::PrivateKey));
    }

    #[test]
    fn detects_github_token() {
        let text = format!("ghp_{}", "a".repeat(36));
        assert_eq!(detect(&text), Some(SensitiveKind::ApiKey));
    }

    #[test]
    fn detects_password_assignment() {
        assert_eq!(
            detect("password: hunter2secret"),
            Some(SensitiveKind::SecretAssignment)
        );
    }

    #[test]
    fn detects_auth_header() {
        assert_eq!(
            detect("authorization: Bearer eyJhbGciOi.payloadhere.signature"),
            Some(SensitiveKind::AuthHeader)
        );
    }

    #[test]
    fn normal_text_is_clean() {
        assert_eq!(detect("Meeting at 3pm tomorrow"), None);
        assert_eq!(detect("https://example.com/docs"), None);
    }
}

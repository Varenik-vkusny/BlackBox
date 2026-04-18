use std::sync::OnceLock;

use regex::Regex;

/// Mask PII and secrets in a log line before it enters the ring buffer.
///
/// Applies in order:
/// 1. Known-format secrets: email, JWT, Bearer token, credit card, password= patterns
/// 2. Known API key prefixes: AWS AKIA, GitHub ghp_/ghs_, Stripe sk-/pk-, OpenAI sk-
/// 3. Entropy scanner: catches unknown high-entropy tokens (API keys, hashes, random tokens)
pub fn mask_pii(text: &str) -> String {
    static EMAIL_RE: OnceLock<Regex> = OnceLock::new();
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    static CREDIT_RE: OnceLock<Regex> = OnceLock::new();
    static PASSWORD_RE: OnceLock<Regex> = OnceLock::new();
    static KNOWN_KEYS_RE: OnceLock<Regex> = OnceLock::new();

    let email_re = EMAIL_RE.get_or_init(|| {
        Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap()
    });
    let jwt_re = JWT_RE.get_or_init(|| {
        // Matches: "Bearer <token>" or bare JWT (eyJ...)
        Regex::new(
            r"(?i)(Bearer\s+)[A-Za-z0-9\-_\.]{20,}|(eyJ[A-Za-z0-9\-_]+\.[A-Za-z0-9\-_]+\.[A-Za-z0-9\-_]*)",
        )
        .unwrap()
    });
    let credit_re = CREDIT_RE.get_or_init(|| {
        // Visa / MC / Amex / Discover — 13-16 digit patterns
        Regex::new(
            r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13}|6(?:011|5[0-9]{2})[0-9]{12})\b",
        )
        .unwrap()
    });
    let password_re = PASSWORD_RE.get_or_init(|| {
        // key=value style credentials
        Regex::new(r"(?i)(?:password|passwd|secret|api[_\-]?key|access[_\-]?token|auth[_\-]?token)\s*[=:]\s*\S+")
            .unwrap()
    });
    let known_keys_re = KNOWN_KEYS_RE.get_or_init(|| {
        // Recognisable API key prefixes from major providers
        Regex::new(r"\b(?:AKIA|ASIA|AROA|AIDA|AGPA|AIPA|ANPA|ANVA|APKA)[A-Z0-9]{16}\b|ghp_[A-Za-z0-9]{36}|ghs_[A-Za-z0-9]{36}|sk-[A-Za-z0-9]{32,}|pk-[A-Za-z0-9]{32,}").unwrap()
    });

    let s = email_re.replace_all(text, "<EMAIL_MASKED>");
    let s = jwt_re.replace_all(&s, |caps: &regex::Captures| {
        if caps.get(1).is_some() {
            // "Bearer <token>" — keep the "Bearer " prefix
            format!("{}<TOKEN_MASKED>", caps.get(1).map_or("", |m| m.as_str()))
        } else {
            "<JWT_MASKED>".to_string()
        }
    });
    let s = credit_re.replace_all(&s, "<CARD_MASKED>");
    let s = password_re.replace_all(&s, |caps: &regex::Captures| {
        let full = caps.get(0).map_or("", |m| m.as_str());
        // Keep "password=" part, mask only the value
        let sep_pos = full.find(['=', ':']).unwrap_or(full.len());
        format!("{}<VALUE_MASKED>", &full[..=sep_pos])
    });
    let s = known_keys_re.replace_all(&s, "<SECRET_MASKED>");

    mask_high_entropy_tokens(&s)
}

/// Shannon entropy of a byte string (bits per character).
fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut freq = [0u32; 256];
    for b in s.bytes() {
        freq[b as usize] += 1;
    }
    let len = s.len() as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .fold(0.0, |acc, &c| {
            let p = c as f64 / len;
            acc - p * p.log2()
        })
}

/// Replace whitespace-delimited tokens that look like secrets (high entropy, long).
fn mask_high_entropy_tokens(text: &str) -> String {
    // Rebuild word-by-word, preserving inter-word whitespace
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for (start, word) in word_spans(text) {
        result.push_str(&text[last_end..start]);
        // Strip trailing punctuation before entropy check
        let stripped = word.trim_end_matches([',', '.', ';', ')', ']', '"', '\'']);
        if stripped.len() >= 20
            && shannon_entropy(stripped) > 4.5
            && !stripped.starts_with("http")
            && !stripped.contains("://")
        {
            result.push_str("<SECRET_MASKED>");
            // Re-add the trailing punctuation
            result.push_str(&word[stripped.len()..]);
        } else {
            result.push_str(word);
        }
        last_end = start + word.len();
    }
    result.push_str(&text[last_end..]);
    result
}

/// Iterator over (byte_offset, &str) for each whitespace-delimited word.
fn word_spans(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut iter = text.char_indices().peekable();
    let mut words: Vec<(usize, &str)> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        words.push((start, &text[start..i]));
    }
    let _ = iter.peek(); // suppress unused warning
    words.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_email() {
        let out = mask_pii("User admin@example.com logged in");
        assert!(out.contains("<EMAIL_MASKED>"), "got: {out}");
        assert!(!out.contains("admin@example.com"));
    }

    #[test]
    fn masks_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let out = mask_pii(&format!("token: {jwt}"));
        assert!(out.contains("<JWT_MASKED>"), "got: {out}");
    }

    #[test]
    fn masks_bearer() {
        let out = mask_pii("Authorization: Bearer ghp_16C7e42F292c6912E7710c838347Ae178B4a");
        assert!(out.contains("Bearer <TOKEN_MASKED>"), "got: {out}");
    }

    #[test]
    fn masks_password_field() {
        let out = mask_pii("config: password=supersecret123");
        assert!(out.contains("password=<VALUE_MASKED>"), "got: {out}");
    }

    #[test]
    fn masks_api_key_field() {
        let out = mask_pii("api_key: sk-live-abc123xyz");
        assert!(out.contains("<VALUE_MASKED>"), "got: {out}");
    }

    #[test]
    fn masks_high_entropy_secret() {
        // AWS-style access key: high entropy, 20+ chars
        let out = mask_pii("key AKIAIOSFODNN7EXAMPLE in config");
        assert!(out.contains("<SECRET_MASKED>"), "got: {out}");
    }

    #[test]
    fn does_not_mask_normal_words() {
        let out = mask_pii("build succeeded in 3.2s");
        assert_eq!(out, "build succeeded in 3.2s");
    }

    #[test]
    fn preserves_url() {
        let out = mask_pii("connecting to https://api.example.com/endpoint");
        assert!(out.contains("https://api.example.com"), "URL should not be masked: {out}");
    }
}

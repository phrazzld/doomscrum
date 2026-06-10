use sha2::{Digest, Sha256};

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn short(hash: &str) -> &str {
    &hash[..hash.len().min(10)]
}

pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_dash = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Deterministic per-spec seed from a content sha256 hex string. Drives
/// script templates, scene ingredients, and render-mix pipeline draws so
/// the same spec always renders the same way.
pub fn spec_seed(sha256_hex: &str) -> u64 {
    u64::from_str_radix(sha256_hex.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable_hex() {
        assert_eq!(sha256_hex(b"doomscrum"), sha256_hex(b"doomscrum"),);
        assert_eq!(sha256_hex(b"").len(), 64);
    }

    #[test]
    fn slug_strips_noise() {
        assert_eq!(slug("Cache Chaos: Exorcism!"), "cache-chaos-exorcism");
        assert_eq!(slug("  --weird__input--  "), "weird-input");
        assert_eq!(slug(""), "");
    }

    #[test]
    fn short_handles_small_input() {
        assert_eq!(short("abc"), "abc");
        assert_eq!(short("0123456789abcdef"), "0123456789");
    }
}

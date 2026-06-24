use hex;
use rand::Rng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Generate a cryptographically secure random token as a hex string.
/// Default 32 bytes produces a 64 hex-char string.
pub fn generate_secure_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    hex::encode(buf)
}

/// SHA-256 hash of a string, returned as lowercase hex.
pub fn sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Base64url-encode bytes (no padding, URL-safe alphabet).
pub fn base64url(input: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(input)
}

/// Timing-safe string comparison using the `subtle` crate.
/// Returns false when lengths differ (runs a dummy comparison to avoid timing leak).
pub fn constant_time_equal(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        // Run a dummy comparison to avoid leaking length via timing
        let _ = a_bytes.ct_eq(a_bytes);
        return false;
    }
    a_bytes.ct_eq(b_bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_length() {
        assert_eq!(generate_secure_token(32).len(), 64);
        assert_eq!(generate_secure_token(16).len(), 32);
    }

    #[test]
    fn sha256_known() {
        assert_eq!(
            sha256("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn constant_time_eq() {
        assert!(constant_time_equal("abc", "abc"));
        assert!(!constant_time_equal("abc", "def"));
        assert!(!constant_time_equal("ab", "abc"));
    }
}

use sha2::{Digest, Sha256};

use super::crypto::{base64url, constant_time_equal};

/// Verify an OAuth 2.0 PKCE code challenge against the original code verifier.
/// RFC 7636: method is "S256" (SHA-256, recommended) or "plain".
pub fn verify_code_challenge(code_verifier: &str, code_challenge: &str, method: &str) -> bool {
    match method {
        "S256" => {
            // BASE64URL(SHA256(ASCII(code_verifier))) per RFC 7636 section 4.6
            let digest = Sha256::digest(code_verifier.as_bytes());
            let computed = base64url(&digest);
            constant_time_equal(&computed, code_challenge)
        }
        "plain" => constant_time_equal(code_verifier, code_challenge),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_verify() {
        // RFC 7636 Appendix B test vector
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(verify_code_challenge(verifier, challenge, "S256"));
        assert!(!verify_code_challenge("wrong", challenge, "S256"));
    }

    #[test]
    fn plain_verify() {
        assert!(verify_code_challenge("abc", "abc", "plain"));
        assert!(!verify_code_challenge("abc", "def", "plain"));
    }
}

use totp_rs::{Algorithm, Secret, TOTP};

use super::crypto::generate_secure_token;

/// Generate a new TOTP secret for a user.
/// Returns (base32_secret, otpauth_uri).
pub fn generate_totp_secret(email: &str, issuer: &str) -> (String, String) {
    let secret = Secret::generate_secret();
    let base32 = secret.to_encoded().to_string();
    let secret_bytes = secret
        .to_bytes()
        .expect("failed to decode generated secret");

    // Build the otpauth URI manually since we don't have the `otpauth` feature
    // Format: otpauth://totp/ISSUER:EMAIL?secret=BASE32&issuer=ISSUER&algorithm=SHA1&digits=6&period=30
    let uri = format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period=30",
        url_encode(issuer),
        url_encode(email),
        base32,
        url_encode(issuer),
    );

    // Verify the secret works by constructing a TOTP (5-param, no otpauth feature)
    let _totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes).expect("failed to create TOTP");

    (base32, uri)
}

/// Verify a TOTP token against a stored base32 secret.
/// Allows a +/-1 step window (30s on each side) to handle clock skew.
pub fn verify_totp(secret_base32: &str, token: &str) -> bool {
    let secret = match Secret::Encoded(secret_base32.to_string()).to_bytes() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let totp = match TOTP::new(Algorithm::SHA1, 6, 1, 30, secret) {
        Ok(t) => t,
        Err(_) => return false,
    };

    // check_current checks the token with the configured skew window
    totp.check_current(token).unwrap_or(false)
}

/// Generate one-time backup recovery codes.
/// Each code is 8 hex characters (4 bytes of entropy).
pub fn generate_backup_codes(count: usize) -> Vec<String> {
    (0..count).map(|_| generate_secure_token(4)).collect()
}

/// Minimal percent-encoding for otpauth URI components.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_generation() {
        let (secret, uri) = generate_totp_secret("user@example.com", "AkurAI IDP");
        assert!(!secret.is_empty());
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("AkurAI"));
    }

    #[test]
    fn backup_codes_count() {
        let codes = generate_backup_codes(10);
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert_eq!(code.len(), 8); // 4 bytes = 8 hex chars
        }
    }
}

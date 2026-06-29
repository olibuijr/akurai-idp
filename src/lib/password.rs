use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};

/// Hash a plaintext password with Argon2id.
/// Returns a PHC-formatted string including salt and parameters.
/// Params: memory=65536 (64 MiB), time=3, parallelism=4.
pub fn hash_password(password: &str) -> Result<String, String> {
    // Generate 16 random bytes for salt, then base64-encode for SaltString
    let mut salt_bytes = [0u8; 16];
    rand::Rng::fill(&mut rand::rng(), &mut salt_bytes);
    // SaltString expects base64-encoded (no padding, charset [a-zA-Z0-9+/])
    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(salt_bytes);
    let salt = SaltString::from_b64(&salt_b64).map_err(|e| e.to_string())?;

    let params = Params::new(65536, 3, 4, None).map_err(|e| e.to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against a stored Argon2id PHC hash.
/// Returns Ok(true) on match, Ok(false) on mismatch, Err on parse failure.
pub fn verify_password(stored_hash: &str, password: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(stored_hash).map_err(|e| e.to_string())?;
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let hash = hash_password("test-password-123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(&hash, "test-password-123").unwrap());
        assert!(!verify_password(&hash, "wrong-password").unwrap());
    }
}

use base64::Engine;
use ed25519_dalek::{pkcs8::DecodePublicKey, SigningKey, VerifyingKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::Value;

use super::crypto::generate_secure_token;

/// Generate a new Ed25519 signing keypair.
/// Returns (kid, public_key_pem, private_key_pem).
/// kid is a 32 hex-char random identifier.
pub fn generate_signing_key() -> (String, String, String) {
    use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};

    // Generate 32 random bytes using rand 0.9 and construct SigningKey directly
    // (avoids rand_core 0.6 vs 0.9 version mismatch with SigningKey::generate)
    let mut secret = [0u8; 32];
    rand::Rng::fill(&mut rand::rng(), &mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();

    let kid = generate_secure_token(16);

    let private_pem = signing_key
        .to_pkcs8_pem(ed25519_dalek::pkcs8::spki::der::pem::LineEnding::LF)
        .expect("failed to encode private key to PEM");
    let public_pem = verifying_key
        .to_public_key_pem(ed25519_dalek::pkcs8::spki::der::pem::LineEnding::LF)
        .expect("failed to encode public key to PEM");

    (kid, public_pem, private_pem.to_string())
}

/// Sign a JWT with EdDSA (Ed25519).
/// `claims` is a JSON object of JWT claims (sub, iss, aud, etc.).
/// `expires_in_secs` is added to the current time for the `exp` claim.
pub fn sign_token(
    claims: &Value,
    private_key_pem: &str,
    kid: &str,
    expires_in_secs: i64,
) -> Result<String, String> {
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some(kid.to_string());

    let now = chrono::Utc::now().timestamp();
    let mut payload = claims.clone();
    if let Some(obj) = payload.as_object_mut() {
        obj.entry("iat").or_insert(Value::from(now));
        obj.entry("exp").or_insert(Value::from(now + expires_in_secs));
    }

    let encoding_key = EncodingKey::from_ed_pem(private_key_pem.as_bytes())
        .map_err(|e| format!("invalid private key: {e}"))?;
    encode(&header, &payload, &encoding_key).map_err(|e| format!("jwt sign error: {e}"))
}

/// Verify a JWT and return its decoded payload.
/// Returns Err if the token is invalid or expired.
pub fn verify_token(token: &str, public_key_pem: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_ed_pem(public_key_pem.as_bytes())
        .map_err(|e| format!("invalid public key: {e}"))?;

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true;
    validation.validate_aud = false;
    validation.required_spec_claims.clear();

    let token_data = decode::<Value>(token, &decoding_key, &validation)
        .map_err(|e| format!("jwt verify error: {e}"))?;
    Ok(token_data.claims)
}

/// Export the public key as a JWK object for a JWKS endpoint.
/// The returned JSON object contains kty, crv, x, kid, alg, use.
pub fn export_public_key_jwk(public_key_pem: &str, kid: &str) -> Result<Value, String> {
    let verifying_key =
        VerifyingKey::from_public_key_pem(public_key_pem).map_err(|e| format!("invalid public key: {e}"))?;

    // Ed25519 JWK: kty=OKP, crv=Ed25519, x=base64url(raw 32-byte public key)
    let x = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifying_key.to_bytes());

    Ok(serde_json::json!({
        "kty": "OKP",
        "crv": "Ed25519",
        "x": x,
        "kid": kid,
        "alg": "EdDSA",
        "use": "sig"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_sign_verify() {
        let (kid, pub_pem, priv_pem) = generate_signing_key();
        let claims = serde_json::json!({"sub": "user-123", "iss": "test"});
        let token = sign_token(&claims, &priv_pem, &kid, 3600).unwrap();
        let decoded = verify_token(&token, &pub_pem).unwrap();
        assert_eq!(decoded["sub"], "user-123");
    }

    #[test]
    fn verifies_token_with_audience_claim() {
        let (kid, pub_pem, priv_pem) = generate_signing_key();
        let claims = serde_json::json!({"sub": "user-123", "iss": "test", "aud": "client-123"});
        let token = sign_token(&claims, &priv_pem, &kid, 3600).unwrap();
        let decoded = verify_token(&token, &pub_pem).unwrap();
        assert_eq!(decoded["aud"], "client-123");
    }

    #[test]
    fn jwk_export() {
        let (_kid, pub_pem, _) = generate_signing_key();
        let jwk = export_public_key_jwk(&pub_pem, "test-kid").unwrap();
        assert_eq!(jwk["kty"], "OKP");
        assert_eq!(jwk["crv"], "Ed25519");
        assert_eq!(jwk["alg"], "EdDSA");
        assert_eq!(jwk["use"], "sig");
        assert_eq!(jwk["kid"], "test-kid");
    }
}

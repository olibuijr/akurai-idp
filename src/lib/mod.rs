pub mod audit;
pub mod crypto;
pub mod html;
pub mod jwt;
pub mod password;
pub mod pkce;
pub mod totp;

/// Parse a DB column that may be a JSON array `["a","b"]` or space-separated `"a b"`.
pub fn parse_json_or_space_separated(s: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| {
        s.split_whitespace().map(String::from).collect()
    })
}

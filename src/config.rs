use std::sync::LazyLock;

pub struct Config {
    pub db_path: String,
    pub listen_addr: String,
    pub base_url: String,
    pub admin_token: String,
    pub agent_public_url: String,
    pub agent_gateway_url: String,
    pub agent_allowed_emails: Vec<String>,
    pub agent_provider: String,
    pub agent_model: String,
    pub port: u16,
}

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let port = std::env::var("IDP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3500u16);
    Config {
        db_path: std::env::var("IDP_DB_PATH").unwrap_or_else(|_| "./data/idp.sqlite".to_string()),
        listen_addr: format!("127.0.0.1:{port}"),
        base_url: std::env::var("IDP_BASE_URL")
            .unwrap_or_else(|_| "https://auth.olibuijr.com".to_string())
            .trim_end_matches('/')
            .to_string(),
        admin_token: std::env::var("IDP_ADMIN_TOKEN").unwrap_or_default(),
        agent_public_url: std::env::var("IDP_AGENT_PUBLIC_URL")
            .unwrap_or_else(|_| "https://agent.olibuijr.com".to_string())
            .trim_end_matches('/')
            .to_string(),
        agent_gateway_url: std::env::var("IDP_AGENT_GATEWAY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8644/query".to_string()),
        agent_allowed_emails: parse_csv_env("IDP_AGENT_ALLOWED_EMAILS", "olibuijr@olibuijr.com"),
        agent_provider: std::env::var("IDP_AGENT_PROVIDER")
            .unwrap_or_else(|_| "openai-codex".to_string()),
        agent_model: std::env::var("IDP_AGENT_MODEL")
            .unwrap_or_else(|_| "gpt-5.4-mini".to_string()),
        port,
    }
});

pub fn get() -> &'static Config {
    &CONFIG
}

fn parse_csv_env(key: &str, default: &str) -> Vec<String> {
    std::env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

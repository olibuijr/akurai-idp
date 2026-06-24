use std::sync::LazyLock;

pub struct Config {
    pub db_path: String,
    pub listen_addr: String,
    pub base_url: String,
    pub admin_token: String,
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
        port,
    }
});

pub fn get() -> &'static Config {
    &CONFIG
}

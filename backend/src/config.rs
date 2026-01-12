use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub rate_limit_enabled: bool,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/bonscompte.db".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .expect("PORT must be a number"),
            // Rate limiting disabled by default - only useful when backend is directly
            // exposed to internet without a reverse proxy. When behind nginx/Caddy/etc,
            // the proxy should handle rate limiting since it sees real client IPs.
            // Backend sees all requests as coming from proxy IP (e.g., 127.0.0.1).
            rate_limit_enabled: env::var("RATE_LIMIT_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        }
    }
}

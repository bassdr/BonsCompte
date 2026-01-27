use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub rate_limit_enabled: bool,
    /// Maximum projects per user (None = unlimited, Some(0) = unlimited, Some(n) = n projects)
    pub max_projects_per_user: Option<i64>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        // Parse max projects per user: None or 0 = unlimited, positive number = limit
        let max_projects_per_user = env::var("MAX_PROJECTS_PER_USER")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|&n| n > 0);

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
            max_projects_per_user,
        }
    }
}

use axum::{http::Method, middleware, routing::get, Router};
use std::{net::SocketAddr, sync::Arc};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use bonscompte_backend::{auth::middleware::JwtSecret, config::Config, db, routes, AppState};

/// Middleware to inject JWT secret and pool into request extensions
async fn inject_extensions(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    request
        .extensions_mut()
        .insert(JwtSecret(state.jwt_secret.clone()));
    request.extensions_mut().insert(state.pool.clone());
    next.run(request).await
}

/// Blocked path prefixes commonly probed by automated scanners
const BLOCKED_PREFIXES: &[&str] = &[
    "/.git",
    "/.env",
    "/.svn",
    "/.hg",
    "/wp-admin",
    "/wp-content",
    "/wp-includes",
    "/wordpress",
    "/phpmyadmin",
    "/pma",
    "/mysql",
    "/admin.php",
    "/config.php",
    "/xmlrpc.php",
    "/eval-stdin.php",
    "/shell",
    "/cgi-bin",
    "/vendor",
    "/node_modules",
    "/.aws",
    "/.docker",
    "/backup",
    "/dump",
    "/debug",
];

/// Blocked exact paths
const BLOCKED_PATHS: &[&str] = &["/robots.txt", "/sitemap.xml", "/favicon.ico"];

/// Blocked file extensions
const BLOCKED_EXTENSIONS: &[&str] = &[
    ".php", ".asp", ".aspx", ".jsp", ".cgi", ".sql", ".bak", ".old", ".zip", ".tar", ".gz", ".rar",
];

/// Middleware to block common scanner probe paths
/// Returns 404 without logging to avoid log noise
async fn block_scan_paths(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = request.uri().path().to_lowercase();

    // Check blocked prefixes
    for prefix in BLOCKED_PREFIXES {
        if path.starts_with(prefix) {
            return axum::http::StatusCode::NOT_FOUND.into_response();
        }
    }

    // Check blocked exact paths
    for blocked in BLOCKED_PATHS {
        if path == *blocked {
            return axum::http::StatusCode::NOT_FOUND.into_response();
        }
    }

    // Check blocked extensions
    for ext in BLOCKED_EXTENSIONS {
        if path.ends_with(ext) {
            return axum::http::StatusCode::NOT_FOUND.into_response();
        }
    }

    next.run(request).await
}

use axum::response::IntoResponse;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,bonscompte_backend=debug".into()),
        )
        .init();

    // Load configuration
    let config = Config::from_env();
    tracing::info!("Starting BonsCompte backend...");
    tracing::info!(
        "Rate limiting: {}",
        if config.rate_limit_enabled {
            "enabled (set RATE_LIMIT_ENABLED=false if behind reverse proxy)"
        } else {
            "disabled (reverse proxy should handle rate limiting)"
        }
    );

    // Initialize database
    let pool = db::init_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    // Run migrations
    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Connected to SQLite at {}", config.database_url);

    // Extract values before moving config into state
    let host = config.host.clone();
    let port = config.port;
    let rate_limit_enabled = config.rate_limit_enabled;

    // Create app state
    let state = AppState {
        pool,
        jwt_secret: config.jwt_secret.clone(),
        config,
    };

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers(Any);

    // Rate limiting configuration - stricter for auth routes
    // Auth: 5 requests per 60 seconds (to prevent brute-force)
    let auth_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(5)
            .finish()
            .expect("Failed to create auth rate limiter"),
    );

    // General API: 100 requests per minute
    let api_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(100)
            .burst_size(100)
            .finish()
            .expect("Failed to create API rate limiter"),
    );

    // Project sub-routes (nested under /api/projects/{id})
    let project_routes = Router::new()
        .nest("/participants", routes::participants::router())
        .nest("/members", routes::members::router())
        .nest("/payments", routes::payments::router())
        .nest("/debts", routes::debts::router())
        .nest("/cashflow", routes::cashflow::router())
        .nest("/history", routes::history::router());

    // Auth routes with optional rate limiting (5 requests per 60s to prevent brute-force)
    let auth_routes = if rate_limit_enabled {
        routes::auth::router().layer(GovernorLayer {
            config: auth_rate_limit,
        })
    } else {
        routes::auth::router()
    };

    // Build router - all routes at root level (use reverse proxy for /api prefix if needed)
    let mut app = Router::new()
        // Health check (no rate limiting)
        .route("/health", get(|| async { "OK" }))
        // Public routes (auth) with optional rate limiting
        .nest("/auth", auth_routes)
        // Recovery routes (some public, some require auth)
        .nest("/recovery", routes::recovery::router())
        // Protected routes (with extensions middleware)
        .nest("/users", routes::users::router())
        .nest("/approvals", routes::approvals::router())
        .nest("/projects", routes::projects::router())
        // Project-scoped routes
        .nest("/projects/{id}", project_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            inject_extensions,
        ));

    // Conditionally apply general rate limiting (100 requests per second)
    if rate_limit_enabled {
        app = app.layer(GovernorLayer {
            config: api_rate_limit,
        });
    }

    let app = app
        // Global middleware
        .layer(TraceLayer::new_for_http())
        // Block scanner probes before logging to reduce noise
        .layer(middleware::from_fn(block_scan_paths))
        .layer(cors)
        .with_state(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid address");

    tracing::info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

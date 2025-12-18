mod auth;
mod config;
mod db;
mod error;
mod models;
mod routes;
mod services;

use axum::{
    extract::FromRef,
    http::Method,
    middleware,
    routing::get,
    Router,
};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use auth::middleware::JwtSecret;
use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: String,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for String {
    fn from_ref(state: &AppState) -> Self {
        state.jwt_secret.clone()
    }
}

/// Middleware to inject JWT secret and pool into request extensions
async fn inject_extensions(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    request.extensions_mut().insert(JwtSecret(state.jwt_secret.clone()));
    request.extensions_mut().insert(state.pool.clone());
    next.run(request).await
}

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

    // Initialize database
    let pool = db::init_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    // Run migrations
    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Connected to SQLite at {}", config.database_url);

    // Create app state
    let state = AppState {
        pool,
        jwt_secret: config.jwt_secret,
    };

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // Project sub-routes (nested under /api/projects/:id)
    let project_routes = Router::new()
        .nest("/participants", routes::participants::router())
        .nest("/members", routes::members::router())
        .nest("/payments", routes::payments::router())
        .nest("/debts", routes::debts::router());

    // Build router - all routes at root level (use reverse proxy for /api prefix if needed)
    let app = Router::new()
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Public routes (auth)
        .nest("/auth", routes::auth::router())
        // Protected routes (with extensions middleware)
        .nest("/users", routes::users::router())
        .nest("/projects", routes::projects::router())
        // Project-scoped routes
        .nest("/projects/{id}", project_routes)
        .layer(middleware::from_fn_with_state(state.clone(), inject_extensions))
        // Global middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid address");

    tracing::info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

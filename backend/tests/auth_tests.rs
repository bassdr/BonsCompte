use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use bonscompte_backend::{auth::handlers, config::Config, db, AppState};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

/// Helper to activate a user (since new users start in pending_approval state)
async fn activate_user(pool: &SqlitePool, username: &str) {
    sqlx::query("UPDATE users SET user_state = 'active' WHERE username = ?")
        .bind(username)
        .execute(pool)
        .await
        .expect("Failed to activate user");
}

/// Helper to create a test app with an in-memory database
async fn create_test_app() -> (Router, SqlitePool) {
    // Use in-memory SQLite database
    let pool = db::init_pool("sqlite::memory:")
        .await
        .expect("Failed to create test database pool");

    // Run migrations
    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    let jwt_secret = "test-secret-key-for-testing".to_string();

    let config = Config {
        database_url: "sqlite::memory:".to_string(),
        jwt_secret: jwt_secret.clone(),
        host: "127.0.0.1".to_string(),
        port: 8000,
        max_projects_per_user: None,
    };

    let state = AppState {
        pool: pool.clone(),
        jwt_secret,
        config,
    };

    let app = Router::new()
        .route("/auth/login", axum::routing::post(handlers::login))
        .route("/auth/register", axum::routing::post(handlers::register))
        .with_state(state);

    (app, pool)
}

#[tokio::test]
async fn test_register_and_login_success() {
    let (app, pool) = create_test_app().await;

    // Register a new user
    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "password123",
                        "display_name": "Test User"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(register_body["token"].is_string());
    assert_eq!(register_body["user"]["username"], "testuser");
    assert_eq!(register_body["user"]["display_name"], "Test User");

    // Activate the user (new users start in pending_approval state)
    activate_user(&pool, "testuser").await;

    // Login with the registered user
    let login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "password123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login_response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(login_body["token"].is_string());
    assert_eq!(login_body["user"]["username"], "testuser");
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    let (app, _pool) = create_test_app().await;

    // Try to login with non-existent user
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "nonexistent",
                        "password": "wrongpassword"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(error_body["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (app, pool) = create_test_app().await;

    // Register a user
    let _register = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "correctpassword"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Activate the user (new users start in pending_approval state)
    activate_user(&pool, "testuser").await;

    // Try to login with wrong password
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "wrongpassword"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(error_body["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let (app, _pool) = create_test_app().await;

    // Register first user
    let _register1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "duplicate",
                        "password": "password123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Try to register with same username
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "duplicate",
                        "password": "anotherpassword"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(error_body["code"], "USERNAME_EXISTS");
}

#[tokio::test]
async fn test_register_password_too_short() {
    let (app, _pool) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "short"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(error_body["code"], "PASSWORD_TOO_WEAK");
}

#[tokio::test]
async fn test_login_returns_user_preferences() {
    let (app, pool) = create_test_app().await;

    // Register a user
    let _register = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "password123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Activate the user (new users start in pending_approval state)
    activate_user(&pool, "testuser").await;

    // Login
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "testuser",
                        "password": "password123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    // Check that user preferences are included with defaults
    assert!(login_body["user"]["preferences"].is_object());
    assert_eq!(login_body["user"]["preferences"]["date_format"], "mdy");
    assert_eq!(login_body["user"]["preferences"]["decimal_separator"], ".");
    assert_eq!(login_body["user"]["preferences"]["currency_symbol"], "$");
    assert_eq!(
        login_body["user"]["preferences"]["currency_symbol_position"],
        "before"
    );
}

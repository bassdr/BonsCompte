use axum::{routing::post, Router};

use crate::auth::handlers::{login, register};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}

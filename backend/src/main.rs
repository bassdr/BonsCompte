use axum::{
    routing::get,
    Json, Router
};
use serde::{Deserialize, Serialize};
use mongodb::{Client, options::ClientOptions};
use std::env;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize)]
struct Test {
    msg: String,
}

#[tokio::main]
async fn main() {
    println!("Intitialize tracing...");
    tracing_subscriber::fmt::init();

    println!("Initialize MongoDB...");
    dotenvy::dotenv().ok();
    let mongo_uri = env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://mongo:27017/bonscompte".to_string());
    let client_options = ClientOptions::parse(&mongo_uri)
        .await
        .expect("Failed to parse Mongo URI");
    let _client = Client::with_options(client_options)
        .expect("Failed to initialize Mongo client");
    println!("Connected to MongoDB at {}", mongo_uri);

    let app = Router::new()
        .route("/", get(|| async { "BonsCompte Rust API running" }))
        .route("/test", get(|| async { Json(Test { msg: "Hello from Rust API!".to_string() }) }));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

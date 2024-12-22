use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

mod api;
mod error;
mod state;
mod websocket;

use state::AppState;

#[tokio::main]
async fn main() {
    // Create database connection pool
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Create app state
    let state = Arc::new(AppState::new(pool));

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    // Build our application with a route
    let app = Router::new()
        // Health check route
        .route("/health", get(health_check))
        // Account routes
        .route("/account/create", post(api::account::create))
        .route("/account/:address/balance", get(api::account::get_balance))
        .route("/account/:address/transactions", get(api::account::get_transactions))
        // Transaction routes
        .route("/transaction/transfer", post(api::transaction::transfer))
        // WebSocket route
        .route("/ws", get(websocket::handler))
        .layer(cors)
        .with_state(state);

    // Start server
    println!("Starting server on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

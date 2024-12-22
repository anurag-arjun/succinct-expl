use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;

use crate::{
    api::{account::*, query::*, transaction::*, websocket::*},
    batch::BatchProcessor,
    websocket::WebSocketState,
};

pub struct AppState {
    pub db: sqlx::PgPool,
    pub updates: broadcast::Sender<WebSocketUpdate>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://localhost/usda_test")
        .await
        .expect("Failed to connect to Postgres");

    // Create WebSocket state
    let websocket_state = Arc::new(WebSocketState::new(pool.clone()));
    let updates = websocket_state.updates.clone();

    // Create app state
    let state = Arc::new(AppState {
        db: pool.clone(),
        updates: updates.clone(),
    });

    // Create batch processor
    let processor = BatchProcessor::new(pool, updates);

    // Spawn batch processor task
    tokio::spawn(async move {
        loop {
            if let Err(e) = processor.process_batch().await {
                tracing::error!("Error processing batch: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    // Build router
    let app = Router::new()
        .route("/accounts/:address/balance", get(get_balance))
        .route("/accounts/:address/nonce", get(get_nonce))
        .route("/transactions", post(submit_transaction))
        .route("/transactions/:tx_id", get(get_transaction))
        .route("/transactions/:tx_id/status", get(get_transaction_status))
        .route("/proofs/:batch_id", get(get_proof_status))
        .route("/proofs", get(list_proofs))
        .route("/ws", get(handle_socket))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

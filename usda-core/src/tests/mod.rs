mod account_tests;
mod transaction_tests;
mod mint_tests;
mod nonce_tests;
mod websocket_tests;
mod util;

use sqlx::PgPool;
use std::sync::Arc;

use crate::state::AppState;

pub async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/usda_test".to_string());
    
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

pub async fn setup_test_state() -> Arc<AppState> {
    let pool = setup_test_db().await;
    Arc::new(AppState::new(pool))
}

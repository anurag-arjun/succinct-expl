pub mod api;
pub mod batch;
pub mod error;
pub mod state;
pub mod websocket;

use sqlx::PgPool;
use tokio::sync::broadcast;
use usda_common::WebSocketUpdate;

pub use error::AppError;
pub use state::AppState;

#[cfg(test)]
mod tests {
    mod util;
    mod account_tests;
    mod transaction_tests;
    mod mint_tests;

    use crate::AppState;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    async fn setup_test_state() -> Arc<AppState> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/usda_test".to_string());

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // Clear database and run migrations
        util::setup_test_database(&pool).await;

        Arc::new(AppState::new(pool))
    }
}

use sqlx::PgPool;
use tokio::sync::broadcast;
use usda_common::{Account, WebSocketMessage};

use crate::error::AppError;

pub struct AppState {
    pub db: PgPool,
    pub ws_tx: broadcast::Sender<WebSocketMessage>,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        let (ws_tx, _) = broadcast::channel(1000); // Buffer size of 1000 messages
        Self {
            db,
            ws_tx,
        }
    }

    pub async fn create_account(&self, public_key: [u8; 32]) -> Result<Account, AppError> {
        let account = sqlx::query_as!(
            Account,
            r#"
            INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            RETURNING 
                address as "address!: [u8; 32]",
                balance as "balance!: i64",
                pending_balance as "pending_balance!: i64",
                nonce as "nonce!: i64",
                created_at as "created_at!"
            "#,
            &public_key[..],
            0_i64,
            0_i64,
            0_i64
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(account)
    }

    pub async fn get_account(&self, address: &[u8; 32]) -> Result<Option<Account>, AppError> {
        let account = sqlx::query_as!(
            Account,
            r#"
            SELECT 
                address as "address!: [u8; 32]",
                balance as "balance!: i64",
                pending_balance as "pending_balance!: i64",
                nonce as "nonce!: i64",
                created_at as "created_at!"
            FROM accounts
            WHERE address = $1
            "#,
            address.as_ref()
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(account)
    }
}

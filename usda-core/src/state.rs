use std::sync::{Arc, Mutex};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;
use ed25519_dalek::VerifyingKey;
use chrono::{DateTime, Utc};

use crate::AppError;
use usda_common::{Account, WebSocketUpdate};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub tx: broadcast::Sender<WebSocketUpdate>,
    issuer_key: Arc<Mutex<Option<VerifyingKey>>>,
}

impl AppState {
    pub fn new(db: PgPool, tx: broadcast::Sender<WebSocketUpdate>) -> Self {
        Self {
            db,
            tx,
            issuer_key: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_issuer_key(&self, key: VerifyingKey) {
        let mut issuer_key = self.issuer_key.lock().unwrap();
        *issuer_key = Some(key);
    }

    pub fn get_issuer_key(&self) -> Option<VerifyingKey> {
        self.issuer_key.lock().unwrap().clone()
    }

    pub async fn get_account(&self, address: [u8; 32]) -> Result<Option<Account>, AppError> {
        let result = sqlx::query_as!(
            Account,
            r#"
            SELECT 
                address as "address!: _",
                balance,
                pending_balance,
                nonce,
                created_at
            FROM accounts
            WHERE address = $1
            "#,
            &address[..],
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(result)
    }

    pub async fn create_account(&self, address: [u8; 32]) -> Result<Account, AppError> {
        let account = sqlx::query_as!(
            Account,
            r#"
            INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
            VALUES ($1, 0, 0, 0, NOW())
            RETURNING 
                address as "address!: _",
                balance,
                pending_balance,
                nonce,
                created_at
            "#,
            &address[..],
        )
        .fetch_one(&self.db)
        .await?;

        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_get_account() {
        let db = PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let (tx, _) = broadcast::channel(100);
        let state = AppState::new(db, tx);

        let address = [0u8; 32];
        let result = state.get_account(address).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_create_account() {
        let db = PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let (tx, _) = broadcast::channel(100);
        let state = AppState::new(db, tx);

        let address = [0u8; 32];
        let account = state.create_account(address).await.unwrap();
        assert_eq!(account.address, address);
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_issuer_key() {
        let db = PgPool::connect_lazy("postgres://localhost/usda_test").unwrap();
        let (tx, _) = broadcast::channel(100);
        let state = AppState::new(db, tx);

        // Initially no key
        assert!(state.get_issuer_key().is_none());

        // Set key
        let key = VerifyingKey::from_bytes(&[0u8; 32]).unwrap();
        state.set_issuer_key(key.clone());

        // Get key back
        let retrieved_key = state.get_issuer_key().unwrap();
        assert_eq!(retrieved_key.to_bytes(), key.to_bytes());
    }
}

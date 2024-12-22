use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use usda_common::{TransactionStatus, WebSocketUpdate};
use uuid::Uuid;
use crate::{AppState, AppError};
use crate::websocket::{publish_proof_update, publish_transaction_update};

pub struct BatchProcessor {
    pub db: Arc<PgPool>,
    pub tx: broadcast::Sender<WebSocketUpdate>,
}

impl BatchProcessor {
    pub fn new(db: Arc<PgPool>, tx: broadcast::Sender<WebSocketUpdate>) -> Self {
        Self { db, tx }
    }

    pub async fn process_batch(&self, batch_id: Uuid) -> Result<bool, AppError> {
        let mut tx = self.db.begin().await?;
        let state = AppState { db: (*self.db).clone(), tx: self.tx.clone() };

        // Get pending transactions
        let rows = sqlx::query!(
            r#"
            SELECT tx_id
            FROM transactions
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT 100
            "#
        )
        .fetch_all(&mut *tx)
        .await?;

        if rows.is_empty() {
            return Ok(false);
        }

        // Process each transaction
        for row in rows {
            // Update balances
            sqlx::query!(
                r#"
                UPDATE accounts a
                SET balance = pending_balance
                FROM transactions t
                WHERE t.tx_id = $1
                AND (a.address = t.from_address OR a.address = t.to_address)
                "#,
                row.tx_id,
            )
            .execute(&mut *tx)
            .await?;

            // Update transaction status
            publish_transaction_update(
                &state,
                row.tx_id,
                TransactionStatus::Executed,
                None,
            )
            .await?;
        }

        // Create proof
        publish_proof_update(
            &state,
            batch_id,
            "completed".to_string(),
            None,
        )
        .await?;

        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_batch_processing() {
        let db = PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let (tx, _) = broadcast::channel(100);
        let processor = BatchProcessor::new(Arc::new(db), tx);

        // Insert test transactions
        let mut tx = processor.db.begin().await.unwrap();
        sqlx::query!(
            r#"
            INSERT INTO transactions (tx_id, status, created_at, updated_at)
            VALUES ($1, 'pending', NOW(), NOW())
            "#,
            Uuid::new_v4(),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Process batch
        let result = processor.process_batch(Uuid::new_v4()).await.unwrap();
        assert!(result);
    }
}

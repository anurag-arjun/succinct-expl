use axum::{
    extract::{State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{AppError, AppState};
use usda_common::{TransactionStatus, ProofStatus};

#[derive(Debug, Serialize)]
pub struct TransactionStatusResponse {
    pub tx_id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProofStatusResponse {
    pub batch_id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TransactionStatusResponse>>, AppError> {
    let transactions = sqlx::query_as!(
        TransactionStatusResponse,
        r#"
        SELECT 
            tx_id as "tx_id!: Uuid",
            status,
            error,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM transactions
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(transactions))
}

pub async fn list_proofs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProofStatusResponse>>, AppError> {
    let proofs = sqlx::query_as!(
        ProofStatusResponse,
        r#"
        SELECT 
            batch_id as "batch_id!: Uuid",
            status,
            error,
            created_at as "created_at!: DateTime<Utc>",
            updated_at as "updated_at!: DateTime<Utc>"
        FROM proofs
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(proofs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_list_transactions() {
        let db = PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let state = Arc::new(AppState::new(db));

        // Insert test transactions
        sqlx::query!(
            r#"
            INSERT INTO transactions (tx_id, status, error, created_at, updated_at)
            VALUES 
                ($1, 'pending', NULL, NOW(), NOW()),
                ($2, 'executed', NULL, NOW(), NOW())
            "#,
            Uuid::new_v4(),
            Uuid::new_v4()
        )
        .execute(&state.db)
        .await
        .unwrap();

        let transactions = list_transactions(State(state)).await.unwrap();
        assert_eq!(transactions.0.len(), 2);
    }

    #[tokio::test]
    async fn test_list_proofs() {
        let db = PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let state = Arc::new(AppState::new(db));
        let batch_id = Uuid::new_v4();

        // Insert test proofs
        sqlx::query!(
            r#"
            INSERT INTO proofs (batch_id, status, error, created_at, updated_at)
            VALUES ($1, 'processing', NULL, NOW(), NOW())
            "#,
            batch_id
        )
        .execute(&state.db)
        .await
        .unwrap();

        let proofs = list_proofs(State(state)).await.unwrap();
        assert_eq!(proofs.0.len(), 1);
        assert_eq!(proofs.0[0].batch_id, batch_id);
        assert_eq!(proofs.0[0].status, "processing");
    }
}

use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};
use usda_common::{TransactionStatus, WebSocketUpdate, TransactionUpdate};

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub from_address: [u8; 32],
    pub to_address: [u8; 32],
    pub amount: i64,
    pub nonce: i64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub tx_id: Uuid,
    pub status: TransactionStatus,
    pub message: Option<String>,
}

pub async fn create_transaction(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TransactionRequest>,
) -> Result<Json<TransactionResponse>, AppError> {
    // Start transaction
    let mut tx = state.db.begin().await?;

    // Validate amount
    if request.amount <= 0 {
        return Err(AppError::InvalidAmount("Transfer amount must be positive".to_string()));
    }

    // Get from account
    let from_account = sqlx::query!(
        r#"
        SELECT balance, nonce
        FROM accounts
        WHERE address = $1
        FOR UPDATE
        "#,
        &request.from_address[..]
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("From account not found".to_string()))?;

    // Check nonce
    if from_account.nonce != request.nonce {
        return Err(AppError::InvalidNonce("Invalid nonce".to_string()));
    }

    // Check balance
    if from_account.balance < request.amount {
        return Err(AppError::InsufficientBalance("Insufficient balance".to_string()));
    }

    // Verify signature
    if let Some(issuer_key) = state.get_issuer_key() {
        // TODO: Implement signature verification
        // if !verify_signature(&request, &issuer_key) {
        //     return Err(AppError::InvalidSignature("Invalid signature".to_string()));
        // }
    }

    // Create transaction record
    let tx_id = Uuid::new_v4();
    let status = TransactionStatus::Processing;

    sqlx::query!(
        r#"
        INSERT INTO transactions (tx_id, from_address, to_address, amount, status)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        tx_id,
        &request.from_address[..],
        &request.to_address[..],
        request.amount,
        status as TransactionStatus,
    )
    .execute(&mut *tx)
    .await?;

    // Update account balances
    sqlx::query!(
        r#"
        UPDATE accounts
        SET balance = balance - $1,
            pending_balance = pending_balance - $1,
            nonce = nonce + 1
        WHERE address = $2
        "#,
        request.amount,
        &request.from_address[..],
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        UPDATE accounts
        SET pending_balance = pending_balance + $1
        WHERE address = $2
        "#,
        request.amount,
        &request.to_address[..],
    )
    .execute(&mut *tx)
    .await?;

    // Commit transaction
    tx.commit().await?;

    // Publish update
    let update = WebSocketUpdate::Transaction(TransactionUpdate {
        tx_id: tx_id.to_string(),
        status,
        message: None,
    });
    let _ = state.tx.send(update);

    Ok(Json(TransactionResponse {
        tx_id,
        status,
        message: None,
    }))
}

pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(tx_id): Path<Uuid>,
) -> Result<Json<TransactionResponse>, AppError> {
    let result = sqlx::query!(
        r#"
        SELECT status, message
        FROM transactions
        WHERE tx_id = $1
        "#,
        tx_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

    Ok(Json(TransactionResponse {
        tx_id,
        status: result.status,
        message: result.message,
    }))
}

async fn update_status(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tx_id: Uuid,
    status: TransactionStatus,
    message: Option<String>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE transactions
        SET status = $1, message = $2
        WHERE tx_id = $3
        "#,
        status as TransactionStatus,
        message,
        tx_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

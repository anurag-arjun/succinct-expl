use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use usda_common::TransactionStatus;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub from: Option<String>,    // hex encoded address
    pub to: String,      // hex encoded address
    pub amount: i64,
    pub fee: i64,
    pub nonce: i64,
    pub signature: String, // hex encoded signature
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub tx_id: String,
    pub status: String,
}

pub async fn transfer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TransferRequest>,
) -> Result<Json<TransactionResponse>, AppError> {
    // Validate amount
    if req.amount <= 0 {
        return Err(AppError::InvalidInput("Transfer amount must be positive".into()));
    }

    // Start a transaction for atomicity
    let mut tx = state.db.begin().await.map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Get sender account if this is not a mint operation
    if let Some(from) = &req.from {
        let from_bytes = hex::decode(from)
            .map_err(|_| AppError::InvalidInput("Invalid from address".into()))?;
        if from_bytes.len() != 32 {
            return Err(AppError::InvalidInput("Invalid from address length".into()));
        }

        let sender = sqlx::query!(
            r#"
            SELECT balance, nonce
            FROM accounts
            WHERE address = $1
            FOR UPDATE
            "#,
            from_bytes.as_slice()
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Sender account not found".into()))?;

        // Verify nonce
        if sender.nonce != req.nonce {
            return Err(AppError::InvalidInput(format!(
                "Invalid nonce. Expected {}, got {}",
                sender.nonce, req.nonce
            )));
        }

        // Verify signature
        let signature_bytes = hex::decode(&req.signature)
            .map_err(|_| AppError::InvalidInput("Invalid signature".into()))?;
        if signature_bytes.len() != 64 {
            return Err(AppError::InvalidInput("Invalid signature length".into()));
        }

        let to_bytes = hex::decode(&req.to)
            .map_err(|_| AppError::InvalidInput("Invalid to address".into()))?;
        if to_bytes.len() != 32 {
            return Err(AppError::InvalidInput("Invalid to address length".into()));
        }

        // Check sufficient balance
        if sender.balance < req.amount + req.fee {
            return Err(AppError::InsufficientBalance);
        }

        // Update sender's balance and nonce
        sqlx::query!(
            r#"
            UPDATE accounts
            SET balance = balance - $1,
                nonce = nonce + 1
            WHERE address = $2
            "#,
            req.amount + req.fee,
            from_bytes.as_slice()
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    }

    // Update receiver's balance
    let to_bytes = hex::decode(&req.to)
        .map_err(|_| AppError::InvalidInput("Invalid to address".into()))?;
    if to_bytes.len() != 32 {
        return Err(AppError::InvalidInput("Invalid to address length".into()));
    }

    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, nonce)
        VALUES ($1, $2, 0)
        ON CONFLICT (address) DO UPDATE
        SET balance = accounts.balance + $2
        "#,
        to_bytes.as_slice(),
        req.amount
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Create transaction record
    let tx_id = Uuid::new_v4().to_string();
    let from_addr = req.from.as_ref().map(|f| hex::decode(f).unwrap());
    
    sqlx::query!(
        r#"
        INSERT INTO transactions (tx_id, from_addr, to_addr, amount, fee, nonce, signature, timestamp, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), $8)
        "#,
        tx_id,
        from_addr.as_deref(),
        to_bytes.as_slice(),
        req.amount,
        req.fee,
        req.nonce,
        hex::decode(&req.signature).unwrap_or_default(),
        TransactionStatus::Pending.to_string()
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(TransactionResponse {
        tx_id,
        status: TransactionStatus::Pending.to_string(),
    }))
}

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use usda_common::{Account, Transaction, TransactionStatus};

use crate::{error::AppError, state::AppState};

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub public_key: [u8; 32], // 32-byte public key
}

#[derive(Serialize)]
pub struct CreateAccountResponse {
    pub address: [u8; 32], // 32-byte address
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<Account>, AppError> {
    let account = state.create_account(req.public_key).await?;

    Ok(Json(account))
}

pub async fn get_balance(
    State(state): State<Arc<AppState>>,
    Path(address): Path<[u8; 32]>,
) -> Result<Json<BalanceResponse>, AppError> {
    let account = state
        .get_account(&address)
        .await?
        .ok_or_else(|| AppError::NotFound("Account not found".into()))?;

    Ok(Json(BalanceResponse {
        balance: account.balance as i64,
        pending_balance: account.pending_balance as i64,
    }))
}

pub async fn get_transactions(
    State(state): State<Arc<AppState>>,
    Path(address): Path<[u8; 32]>,
) -> Result<Json<Vec<Transaction>>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT 
            tx_id,
            from_addr as "from_addr?: Vec<u8>",
            to_addr as "to_addr!: Vec<u8>", 
            amount as "amount!: i64", 
            fee as "fee!: i64", 
            nonce as "nonce!: i64", 
            signature as "signature!: Vec<u8>", 
            timestamp as "timestamp!", 
            status as "status!"
        FROM transactions 
        WHERE from_addr = $1 OR to_addr = $1
        ORDER BY timestamp DESC
        "#,
        &address[..]
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let transactions = rows
        .into_iter()
        .map(|row| {
            let status_str = row.status;
            let status = match status_str.as_str() {
                "pending" => TransactionStatus::Pending,
                "processing" => TransactionStatus::Processing,
                "executed" => TransactionStatus::Executed,
                "failed" => TransactionStatus::Failed,
                _ => TransactionStatus::Pending,
            };
            Transaction {
                tx_id: row.tx_id,
                from: row.from_addr.map(|addr| addr.try_into().unwrap()),
                to: row.to_addr[..].try_into().unwrap(),
                amount: row.amount,
                fee: row.fee,
                nonce: row.nonce,
                signature: row.signature[..].try_into().unwrap(),
                timestamp: row.timestamp,
                status,
            }
        })
        .collect();

    Ok(Json(transactions))
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub balance: i64,
    pub pending_balance: i64,
}

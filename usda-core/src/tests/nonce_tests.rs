use axum::Json;
use ed25519_dalek::{SigningKey, Signer};
use rand::rngs::OsRng;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::broadcast;
use usda_common::WebSocketMessage;

use crate::{
    api::transaction::{transfer, TransferRequest},
    error::AppError,
    state::AppState,
};

async fn setup_test_state() -> Arc<AppState> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://localhost/usda_test")
        .await
        .expect("Failed to create connection pool");

    sqlx::query!("DELETE FROM transactions")
        .execute(&pool)
        .await
        .expect("Failed to clear transactions");
    sqlx::query!("DELETE FROM accounts")
        .execute(&pool)
        .await
        .expect("Failed to clear accounts");

    let (_tx, _) = broadcast::channel::<WebSocketMessage>(1000);
    Arc::new(AppState::new(pool))
}

#[tokio::test]
async fn test_nonce_validation() {
    let state = setup_test_state().await;

    // Create two accounts
    let mut sender_secret = [0u8; 32];
    OsRng.fill_bytes(&mut sender_secret);
    let sender_signing_key = SigningKey::from_bytes(&sender_secret);
    let sender_verifying_key = sender_signing_key.verifying_key();
    let sender_address = sender_verifying_key.to_bytes();

    let mut recipient_secret = [0u8; 32];
    OsRng.fill_bytes(&mut recipient_secret);
    let recipient_signing_key = SigningKey::from_bytes(&recipient_secret);
    let recipient_verifying_key = recipient_signing_key.verifying_key();
    let recipient_address = recipient_verifying_key.to_bytes();

    // Create accounts with initial balance
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        sender_address.as_slice(),
        1000_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create sender account");

    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        recipient_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create recipient account");

    // Test 1: Valid nonce should succeed
    let transfer_message = format!(
        "{}{}{}{}",
        hex::encode(sender_address),
        hex::encode(recipient_address),
        100,
        0 // First nonce
    );
    let transfer_signature = sender_signing_key.sign(transfer_message.as_bytes());

    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(sender_address)),
        to: hex::encode(recipient_address),
        amount: 100,
        nonce: 0,
        signature: hex::encode(transfer_signature.to_bytes()),
    });

    let result = transfer(axum::extract::State(state.clone()), transfer_req)
        .await;
    assert!(result.is_ok(), "First transfer with nonce 0 should succeed");

    // Test 2: Reusing the same nonce should fail
    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(sender_address)),
        to: hex::encode(recipient_address),
        amount: 100,
        nonce: 0, // Reusing nonce
        signature: hex::encode(transfer_signature.to_bytes()),
    });

    let result = transfer(axum::extract::State(state.clone()), transfer_req)
        .await;
    assert!(matches!(result, Err(AppError::InvalidNonce)), "Reused nonce should fail");

    // Test 3: Skipping a nonce should fail
    let transfer_message = format!(
        "{}{}{}{}",
        hex::encode(sender_address),
        hex::encode(recipient_address),
        100,
        2 // Skipping nonce 1
    );
    let transfer_signature = sender_signing_key.sign(transfer_message.as_bytes());

    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(sender_address)),
        to: hex::encode(recipient_address),
        amount: 100,
        nonce: 2,
        signature: hex::encode(transfer_signature.to_bytes()),
    });

    let result = transfer(axum::extract::State(state.clone()), transfer_req)
        .await;
    assert!(matches!(result, Err(AppError::InvalidNonce)), "Skipped nonce should fail");

    // Test 4: Correct next nonce should succeed
    let transfer_message = format!(
        "{}{}{}{}",
        hex::encode(sender_address),
        hex::encode(recipient_address),
        100,
        1 // Correct next nonce
    );
    let transfer_signature = sender_signing_key.sign(transfer_message.as_bytes());

    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(sender_address)),
        to: hex::encode(recipient_address),
        amount: 100,
        nonce: 1,
        signature: hex::encode(transfer_signature.to_bytes()),
    });

    let result = transfer(axum::extract::State(state.clone()), transfer_req)
        .await;
    assert!(result.is_ok(), "Transfer with correct next nonce should succeed");

    // Test 5: Different accounts should have independent nonces
    let mut other_secret = [0u8; 32];
    OsRng.fill_bytes(&mut other_secret);
    let other_signing_key = SigningKey::from_bytes(&other_secret);
    let other_verifying_key = other_signing_key.verifying_key();
    let other_address = other_verifying_key.to_bytes();

    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        other_address.as_slice(),
        1000_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create other account");

    let transfer_message = format!(
        "{}{}{}{}",
        hex::encode(other_address),
        hex::encode(recipient_address),
        100,
        0 // First nonce for new account
    );
    let transfer_signature = other_signing_key.sign(transfer_message.as_bytes());

    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(other_address)),
        to: hex::encode(recipient_address),
        amount: 100,
        nonce: 0,
        signature: hex::encode(transfer_signature.to_bytes()),
    });

    let result = transfer(axum::extract::State(state.clone()), transfer_req)
        .await;
    assert!(result.is_ok(), "Transfer from different account with nonce 0 should succeed");
}

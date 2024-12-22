use super::*;
use crate::api::transaction::{transfer, TransferRequest};
use axum::Json;
use axum::extract::State;
use crate::error::AppError;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, SecretKey};
use rand::{RngCore, rngs::OsRng};

async fn setup_test_accounts(state: &AppState) -> (SigningKey, VerifyingKey) {
    // Create sender and receiver keypairs
    let mut sender_secret = [0u8; 32];
    OsRng.fill_bytes(&mut sender_secret);
    let sender_signing_key = SigningKey::from_bytes(&sender_secret);
    let sender_verifying_key = sender_signing_key.verifying_key();
    
    let mut receiver_secret = [0u8; 32];
    OsRng.fill_bytes(&mut receiver_secret);
    let receiver_signing_key = SigningKey::from_bytes(&receiver_secret);
    let receiver_verifying_key = receiver_signing_key.verifying_key();
    
    // Store bytes for later use
    let sender_bytes = sender_verifying_key.to_bytes();
    let receiver_bytes = receiver_verifying_key.to_bytes();
    
    // Insert sender account with balance
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        sender_bytes.as_slice(),
        1000_i64,
        1000_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to insert sender account");
    
    // Insert receiver account
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        receiver_bytes.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to insert receiver account");
    
    (sender_signing_key, receiver_verifying_key)
}

#[tokio::test]
async fn test_transfer() {
    // Set up test state
    let state = setup_test_state().await;
    
    // Set up issuer key
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let issuer_key = SigningKey::from_bytes(&secret_bytes);
    state.set_issuer_key(issuer_key.verifying_key());
    
    // Create sender and receiver keypairs
    let (sender_signing_key, receiver_verifying_key) = setup_test_accounts(&state).await;
    
    // Create transfer request
    let amount = 100_i64;
    let nonce = 0_i64;
    
    // Store bytes for later use
    let sender_bytes = sender_signing_key.verifying_key().to_bytes();
    let receiver_bytes = receiver_verifying_key.to_bytes();
    
    // Create message to sign
    let message = format!(
        "{}{}{}{}",
        hex::encode(sender_bytes),
        hex::encode(receiver_bytes),
        amount,
        nonce
    );
    
    // Sign message
    let signature = sender_signing_key.sign(message.as_bytes());
    
    let req = Json(TransferRequest {
        from: Some(hex::encode(sender_bytes)),
        to: hex::encode(receiver_bytes),
        amount,
        fee: amount / 100, // 1% fee
        nonce,
        signature: hex::encode(signature.to_bytes()),
    });
    
    // Execute transfer
    let response = transfer(axum::extract::State(state.clone()), req)
        .await
        .expect("Failed to execute transfer");
    
    // Verify response
    assert!(!response.0.tx_id.is_empty());
    assert_eq!(response.0.status, "pending");
    
    // Verify balances
    let sender = sqlx::query!(
        "SELECT balance FROM accounts WHERE address = $1",
        &sender_bytes
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let receiver = sqlx::query!(
        "SELECT balance FROM accounts WHERE address = $1",
        &receiver_bytes
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    // Account for 1% fee
    let fee = amount / 100;
    assert_eq!(sender.balance, 1000 - amount - fee);  // Initial balance - amount - fee
    assert_eq!(receiver.balance, amount);  // Received full amount
}

#[tokio::test]
async fn test_transfer_insufficient_balance() {
    let state = setup_test_state().await;
    let (sender_signing_key, receiver_verifying_key) = setup_test_accounts(&state).await;
    
    // Create transfer request with amount larger than balance
    let amount = 2000_i64; // Balance is only 1000
    let nonce = 0_i64;
    
    // Store bytes for later use
    let sender_bytes = sender_signing_key.verifying_key().to_bytes();
    let receiver_bytes = receiver_verifying_key.to_bytes();
    
    let message = format!(
        "{}{}{}{}",
        hex::encode(sender_bytes),
        hex::encode(receiver_bytes),
        amount,
        nonce
    );
    
    let signature = sender_signing_key.sign(message.as_bytes());
    
    let req = Json(TransferRequest {
        from: Some(hex::encode(sender_bytes)),
        to: hex::encode(receiver_bytes),
        amount,
        fee: amount / 100, // 1% fee
        nonce,
        signature: hex::encode(signature.to_bytes()),
    });
    
    // Execute transfer
    let result = transfer(axum::extract::State(state.clone()), req).await;
    
    // Verify it fails with insufficient balance
    assert!(matches!(result, Err(crate::error::AppError::InsufficientBalance)));
}

#[tokio::test]
async fn test_transfer_zero_amount() {
    let state = setup_test_state().await;
    
    // Set up issuer key
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let issuer_key = SigningKey::from_bytes(&secret_bytes);
    state.set_issuer_key(issuer_key.verifying_key());
    
    // Create sender and receiver keypairs
    let (sender_signing_key, receiver_verifying_key) = setup_test_accounts(&state).await;
    
    // Create transfer request with zero amount
    let amount = 0;
    let nonce = 0;
    let fee = 0;
    
    let sender_bytes = sender_signing_key.verifying_key().to_bytes();
    let receiver_bytes = receiver_verifying_key.to_bytes();
    
    let message = format!("{}:{}:{}", hex::encode(&receiver_bytes), amount, nonce);
    let signature = sender_signing_key.sign(message.as_bytes());
    
    let req = Json(TransferRequest {
        from: Some(hex::encode(sender_bytes)),
        to: hex::encode(receiver_bytes),
        amount,
        fee,
        nonce,
        signature: hex::encode(signature.to_bytes()),
    });
    
    // Attempt transfer
    let result = transfer(State(state), req).await;
    assert!(result.is_err(), "Should fail with zero amount");
    match result {
        Err(AppError::InvalidInput(msg)) => assert!(msg.contains("amount")),
        _ => panic!("Expected InvalidInput error"),
    }
}

#[tokio::test]
async fn test_concurrent_transfers() {
    let state = setup_test_state().await;
    
    // Set up issuer key
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let issuer_key = SigningKey::from_bytes(&secret_bytes);
    state.set_issuer_key(issuer_key.verifying_key());
    
    // Create sender and two receiver keypairs
    let (sender_signing_key, receiver1_verifying_key) = setup_test_accounts(&state).await;
    let mut receiver2_secret = [0u8; 32];
    OsRng.fill_bytes(&mut receiver2_secret);
    let receiver2_signing_key = SigningKey::from_bytes(&receiver2_secret);
    let receiver2_verifying_key = receiver2_signing_key.verifying_key();
    
    // Create second receiver account
    let receiver2_bytes = receiver2_verifying_key.to_bytes();
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        receiver2_bytes.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .unwrap();
    
    // Prepare concurrent transfers
    let sender_bytes = sender_signing_key.verifying_key().to_bytes();
    let receiver1_bytes = receiver1_verifying_key.to_bytes();
    
    let amount = 100;
    let fee = amount / 100;
    let nonce = 0;
    
    // Create two transfer requests with same nonce
    let message1 = format!("{}:{}:{}", hex::encode(&receiver1_bytes), amount, nonce);
    let signature1 = sender_signing_key.sign(message1.as_bytes());
    
    let message2 = format!("{}:{}:{}", hex::encode(&receiver2_bytes), amount, nonce);
    let signature2 = sender_signing_key.sign(message2.as_bytes());
    
    let req1 = Json(TransferRequest {
        from: Some(hex::encode(sender_bytes)),
        to: hex::encode(receiver1_bytes),
        amount,
        fee,
        nonce,
        signature: hex::encode(signature1.to_bytes()),
    });
    
    let req2 = Json(TransferRequest {
        from: Some(hex::encode(sender_bytes)),
        to: hex::encode(receiver2_bytes),
        amount,
        fee,
        nonce,
        signature: hex::encode(signature2.to_bytes()),
    });
    
    // Execute transfers concurrently
    let state_clone = state.clone();
    let (res1, res2) = tokio::join!(
        transfer(State(state.clone()), req1),
        transfer(State(state_clone), req2)
    );
    
    // One should succeed, one should fail with nonce error
    assert!(res1.is_ok() != res2.is_ok(), "One transfer should succeed, one should fail");
    match (res1, res2) {
        (Ok(_), Err(AppError::InvalidNonce)) | (Err(AppError::InvalidNonce), Ok(_)) => (),
        _ => panic!("Expected one success and one InvalidNonce error"),
    }
}

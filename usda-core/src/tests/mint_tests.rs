use super::*;
use crate::api::transaction::{mint, MintRequest};
use axum::Json;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::{rngs::OsRng, RngCore};

async fn setup_test_accounts(state: &AppState) -> (SigningKey, VerifyingKey) {
    // Generate issuer keypair
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    
    // Set issuer key
    state.set_issuer_key(verifying_key);
    
    (signing_key, verifying_key)
}

#[tokio::test]
async fn test_mint() {
    let state = setup_test_state().await;
    
    // Setup test accounts
    let (signing_key, _) = setup_test_accounts(&state).await;
    
    // Generate recipient keypair
    let mut receiver_secret = [0u8; 32];
    OsRng.fill_bytes(&mut receiver_secret);
    let receiver_signing_key = SigningKey::from_bytes(&receiver_secret);
    let receiver_verifying_key = receiver_signing_key.verifying_key();
    let receiver_address = receiver_verifying_key.to_bytes();

    // Create recipient account
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        receiver_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create recipient account");
    
    // Create mint request
    let amount = 100_i64;
    
    // Create message to sign (to + amount)
    let message = format!(
        "{}{}",
        hex::encode(receiver_address),
        amount
    );
    
    // Sign message
    let signature = signing_key.sign(message.as_bytes());
    
    let req = Json(MintRequest {
        to: hex::encode(receiver_address),
        amount,
        signature: hex::encode(signature.to_bytes()),
    });
    
    // Execute mint
    let response = mint(axum::extract::State(state.clone()), req)
        .await
        .expect("Failed to execute mint");
    
    // Verify response
    assert!(!response.0.tx_id.is_empty());
    assert_eq!(response.0.status, "pending");
    
    // Verify balances
    let receiver = sqlx::query!(
        "SELECT pending_balance FROM accounts WHERE address = $1",
        receiver_address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to fetch receiver account");
    
    // Verify receiver's balance is increased by amount
    assert_eq!(receiver.pending_balance, amount);
}

#[tokio::test]
async fn test_mint_invalid_signature() {
    let state = setup_test_state().await;
    
    // Setup test accounts
    let (_, _) = setup_test_accounts(&state).await;
    
    // Generate recipient keypair
    let mut receiver_secret = [0u8; 32];
    OsRng.fill_bytes(&mut receiver_secret);
    let receiver_signing_key = SigningKey::from_bytes(&receiver_secret);
    let receiver_verifying_key = receiver_signing_key.verifying_key();
    let receiver_address = receiver_verifying_key.to_bytes();

    // Create recipient account
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        receiver_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create recipient account");
    
    // Create mint request
    let amount = 100_i64;
    
    // Create message to sign (to + amount)
    let message = format!(
        "{}{}",
        hex::encode(receiver_address),
        amount
    );
    
    // Sign message with wrong key
    let mut wrong_secret = [0u8; 32];
    OsRng.fill_bytes(&mut wrong_secret);
    let wrong_signing_key = SigningKey::from_bytes(&wrong_secret);
    let signature = wrong_signing_key.sign(message.as_bytes());
    
    let req = Json(MintRequest {
        to: hex::encode(receiver_address),
        amount,
        signature: hex::encode(signature.to_bytes()),
    });
    
    // Execute mint
    let result = mint(axum::extract::State(state.clone()), req).await;
    
    // Verify it fails with invalid signature
    assert!(matches!(result, Err(crate::error::AppError::InvalidSignature)));
}

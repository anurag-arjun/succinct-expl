use super::*;
use crate::api::account::{create, get_balance, CreateAccountRequest};
use axum::{extract::Path, Json};
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};

#[tokio::test]
async fn test_create_account() {
    let state = setup_test_state().await;
    
    // Generate a keypair
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let address = signing_key.verifying_key().to_bytes();
    
    // Create account request
    let req = Json(CreateAccountRequest {
        public_key: address,
    });
    
    // Create account
    let _response = create(axum::extract::State(state.clone()), req)
        .await
        .expect("Failed to create account");
    
    // Verify account was created
    let account = sqlx::query!(
        r#"
        SELECT address, balance, pending_balance
        FROM accounts
        WHERE address = $1
        "#,
        address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to fetch account");
    
    assert_eq!(account.balance, 0);
    assert_eq!(account.pending_balance, 0);
}

#[tokio::test]
async fn test_get_balance() {
    let state = setup_test_state().await;
    
    // Generate a keypair
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let address = signing_key.verifying_key().to_bytes();
    
    // Insert account with balance
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        address.as_slice(),
        1000_i64,
        1000_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to insert account");
    
    // Get balance
    let response = get_balance(
        axum::extract::State(state.clone()),
        Path(address),
    )
    .await
    .expect("Failed to get balance");
    
    // Verify balance
    assert_eq!(response.0.balance, 1000);
    assert_eq!(response.0.pending_balance, 1000);
}

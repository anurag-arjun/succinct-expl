use super::*;
use crate::api::transaction::{transfer, TransferRequest};
use axum::{
    extract::State,
    extract::ws::{Message, WebSocket},
    Json,
};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use futures::{SinkExt, StreamExt};
use rand::{RngCore, rngs::OsRng};
use tokio::sync::broadcast;
use usda_common::WebSocketMessage;

#[tokio::test]
async fn test_websocket_notifications() {
    let state = setup_test_state().await;
    
    // Set up issuer key
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let issuer_key = SigningKey::from_bytes(&secret_bytes);
    state.set_issuer_key(issuer_key.verifying_key());
    
    // Create test accounts
    let (sender_signing_key, receiver_verifying_key) = setup_test_accounts(&state).await;
    let sender_bytes = sender_signing_key.verifying_key().to_bytes();
    let receiver_bytes = receiver_verifying_key.to_bytes();
    
    // Create WebSocket connection
    let (tx, mut rx) = broadcast::channel(100);
    
    // Subscribe to both accounts
    let sender_sub = format!("account:{}", hex::encode(&sender_bytes));
    let receiver_sub = format!("account:{}", hex::encode(&receiver_bytes));
    
    // Create transfer
    let amount = 100;
    let fee = amount / 100;
    let nonce = 0;
    
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
    
    // Execute transfer
    let response = transfer(State(state.clone()), req).await.unwrap();
    
    // Verify WebSocket messages
    let msg = rx.try_recv().unwrap();
    match msg {
        WebSocketMessage::BalanceUpdate { address, balance, pending_balance } => {
            assert_eq!(hex::encode(address), hex::encode(sender_bytes));
            assert_eq!(balance, 1000 - amount - fee);
            assert_eq!(pending_balance, 0);
        }
    }
    
    let msg = rx.try_recv().unwrap();
    match msg {
        WebSocketMessage::BalanceUpdate { address, balance, pending_balance } => {
            assert_eq!(hex::encode(address), hex::encode(receiver_bytes));
            assert_eq!(balance, amount);
            assert_eq!(pending_balance, 0);
        }
    }
}

#[tokio::test]
async fn test_websocket_reconnection() {
    let state = setup_test_state().await;
    
    // Set up issuer key
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let issuer_key = SigningKey::from_bytes(&secret_bytes);
    state.set_issuer_key(issuer_key.verifying_key());
    
    // Create test account
    let (signing_key, _) = setup_test_accounts(&state).await;
    let account_bytes = signing_key.verifying_key().to_bytes();
    
    // Create WebSocket channels
    let (tx1, mut rx1) = broadcast::channel(100);
    let (tx2, mut rx2) = broadcast::channel(100);
    
    // Subscribe to account on both channels
    let account_sub = format!("account:{}", hex::encode(&account_bytes));
    
    // Simulate disconnection by dropping rx1
    drop(rx1);
    
    // Subscribe with new channel
    let account_sub = format!("account:{}", hex::encode(&account_bytes));
    
    // Verify new channel receives updates
    let amount = 100;
    let fee = amount / 100;
    let nonce = 0;
    
    // Update balance
    sqlx::query!(
        r#"
        UPDATE accounts
        SET balance = balance + $1
        WHERE address = $2
        "#,
        amount,
        account_bytes.as_slice()
    )
    .execute(&state.db)
    .await
    .unwrap();
    
    // Verify only rx2 receives the update
    let msg = rx2.try_recv().unwrap();
    match msg {
        WebSocketMessage::BalanceUpdate { address, balance, .. } => {
            assert_eq!(hex::encode(address), hex::encode(account_bytes));
            assert_eq!(balance, 1000 + amount);
        }
    }
}

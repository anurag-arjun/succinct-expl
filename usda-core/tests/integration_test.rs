use axum::{extract::State, Json};
use ed25519_dalek::{Signer, SigningKey};
use rand::{rngs::OsRng, RngCore};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::broadcast;
use usda_common::WebSocketMessage;
use usda_core::{
    api::{
        account::CreateAccountRequest,
        transaction::{mint, transfer, MintRequest, TransferRequest},
    },
    state::AppState,
};

async fn setup_test_state() -> Arc<AppState> {
    // Create a connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://localhost/usda_test")
        .await
        .expect("Failed to create connection pool");

    // Clear any existing data
    sqlx::query!("DELETE FROM transactions")
        .execute(&pool)
        .await
        .expect("Failed to clear transactions");
    sqlx::query!("DELETE FROM accounts")
        .execute(&pool)
        .await
        .expect("Failed to clear accounts");

    // Create broadcast channel for WebSocket messages
    let (_tx, _) = broadcast::channel::<WebSocketMessage>(100);

    // Create app state
    Arc::new(AppState::new(pool))
}

async fn confirm_pending_transactions(state: &AppState, issuer_address: &[u8; 32]) {
    // Get all pending transactions
    let pending_txs = sqlx::query!(
        "SELECT tx_id, from_addr, to_addr, amount, fee FROM transactions WHERE status = 'PENDING'"
    )
    .fetch_all(&state.db)
    .await
    .expect("Failed to fetch pending transactions");

    println!("Found {} pending transactions", pending_txs.len());

    // Confirm each transaction
    for tx in pending_txs {
        println!("Confirming transaction {} for amount {}", tx.tx_id, tx.amount);

        // Update transaction status
        sqlx::query!(
            "UPDATE transactions SET status = 'CONFIRMED' WHERE tx_id = $1",
            tx.tx_id
        )
        .execute(&state.db)
        .await
        .expect("Failed to update transaction status");

        // Update balances
        if let Some(from_addr) = tx.from_addr {
            // For transfers, subtract from sender's balance
            sqlx::query!(
                r#"
                UPDATE accounts 
                SET balance = balance - $1,
                    pending_balance = pending_balance + $1
                WHERE address = $2
                "#,
                tx.amount + tx.fee,
                from_addr
            )
            .execute(&state.db)
            .await
            .expect("Failed to update sender balance");

            // Add fee to issuer's balance
            if tx.fee > 0 {
                sqlx::query!(
                    r#"
                    UPDATE accounts 
                    SET balance = balance + $1
                    WHERE address = $2
                    "#,
                    tx.fee,
                    issuer_address.as_slice()
                )
                .execute(&state.db)
                .await
                .expect("Failed to update issuer balance");
            }
        }

        // Add to recipient's balance
        sqlx::query!(
            r#"
            UPDATE accounts 
            SET balance = balance + $1,
                pending_balance = pending_balance - $1
            WHERE address = $2
            "#,
            tx.amount,
            tx.to_addr
        )
        .execute(&state.db)
        .await
        .expect("Failed to update recipient balance");
    }
}

#[tokio::test]
async fn test_payment_flow() {
    let state = setup_test_state().await;

    // Generate issuer keypair
    let mut issuer_secret = [0u8; 32];
    OsRng.fill_bytes(&mut issuer_secret);
    let issuer_signing_key = SigningKey::from_bytes(&issuer_secret);
    let issuer_verifying_key = issuer_signing_key.verifying_key();
    let issuer_address = issuer_verifying_key.to_bytes();
    state.set_issuer_key(issuer_verifying_key);

    // Create issuer account
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        issuer_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create issuer account");

    // Generate Alice's keypair
    let mut alice_secret = [0u8; 32];
    OsRng.fill_bytes(&mut alice_secret);
    let alice_signing_key = SigningKey::from_bytes(&alice_secret);
    let alice_verifying_key = alice_signing_key.verifying_key();
    let alice_address = alice_verifying_key.to_bytes();

    // Generate Bob's keypair
    let mut bob_secret = [0u8; 32];
    OsRng.fill_bytes(&mut bob_secret);
    let bob_signing_key = SigningKey::from_bytes(&bob_secret);
    let bob_verifying_key = bob_signing_key.verifying_key();
    let bob_address = bob_verifying_key.to_bytes();

    // 1. Create accounts for Alice and Bob
    let _alice_req = Json(CreateAccountRequest {
        public_key: alice_address,
    });
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        alice_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create Alice's account");

    let _bob_req = Json(CreateAccountRequest {
        public_key: bob_address,
    });
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, pending_balance, nonce, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        bob_address.as_slice(),
        0_i64,
        0_i64,
        0_i64
    )
    .execute(&state.db)
    .await
    .expect("Failed to create Bob's account");

    // 2. Mint 1000 tokens to Alice's account
    let mint_amount = 1000_i64;
    let mint_message = format!("{}{}", hex::encode(alice_address), mint_amount);
    let mint_signature = issuer_signing_key.sign(mint_message.as_bytes());

    let mint_req = Json(MintRequest {
        to: hex::encode(alice_address),
        amount: mint_amount,
        signature: hex::encode(mint_signature.to_bytes()),
    });
    let _ = mint(State(state.clone()), mint_req)
        .await
        .expect("Failed to mint tokens to Alice");

    // Check if minting transaction was created
    let pending_txs = sqlx::query!(
        "SELECT tx_id, from_addr, to_addr, amount, status FROM transactions"
    )
    .fetch_all(&state.db)
    .await
    .expect("Failed to fetch pending transactions");
    println!("All transactions: {}", pending_txs.len());
    for tx in &pending_txs {
        println!(
            "Transaction {} from {:?} to {} for amount {} (status: {})",
            tx.tx_id,
            tx.from_addr.as_ref().map(hex::encode),
            hex::encode(&tx.to_addr),
            tx.amount,
            tx.status
        );
    }

    // Wait for mint transaction to be confirmed
    confirm_pending_transactions(&state, &issuer_address).await;

    // 3. Check Alice's balance
    let alice_balance = sqlx::query!(
        "SELECT balance, pending_balance FROM accounts WHERE address = $1",
        alice_address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to get Alice's balance");
    println!(
        "After mint: Alice's balance = {}, pending_balance = {}",
        alice_balance.balance, alice_balance.pending_balance
    );
    assert_eq!(alice_balance.balance, mint_amount);

    // 4. Transfer 500 tokens from Alice to Bob
    let transfer_amount = 500_i64;
    let transfer_nonce = 0_i64;
    let transfer_message = format!(
        "{}{}{}{}",
        hex::encode(alice_address),
        hex::encode(bob_address),
        transfer_amount,
        transfer_nonce
    );
    let transfer_signature = alice_signing_key.sign(transfer_message.as_bytes());

    let transfer_req = Json(TransferRequest {
        from: Some(hex::encode(alice_address)),
        to: hex::encode(bob_address),
        amount: transfer_amount,
        fee: transfer_amount / 100, // 1% fee
        nonce: transfer_nonce,
        signature: hex::encode(transfer_signature.to_bytes()),
    });
    let _ = transfer(State(state.clone()), transfer_req)
        .await
        .expect("Failed to transfer tokens from Alice to Bob");

    // Wait for transfer transaction to be confirmed
    confirm_pending_transactions(&state, &issuer_address).await;

    // 5. Check final balances
    let alice_final_balance = sqlx::query!(
        "SELECT balance, pending_balance FROM accounts WHERE address = $1",
        alice_address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to get Alice's final balance");
    println!(
        "After transfer: Alice's balance = {}, pending_balance = {}",
        alice_final_balance.balance, alice_final_balance.pending_balance
    );
    assert_eq!(
        alice_final_balance.balance,
        mint_amount - transfer_amount - (transfer_amount / 100)
    ); // Subtract amount + 1% fee

    let bob_final_balance = sqlx::query!(
        "SELECT balance, pending_balance FROM accounts WHERE address = $1",
        bob_address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to get Bob's final balance");
    println!(
        "After transfer: Bob's balance = {}, pending_balance = {}",
        bob_final_balance.balance, bob_final_balance.pending_balance
    );
    assert_eq!(bob_final_balance.balance, transfer_amount);

    // Check issuer's balance (should have collected the fee)
    let issuer_final_balance = sqlx::query!(
        "SELECT balance FROM accounts WHERE address = $1",
        issuer_address.as_slice()
    )
    .fetch_one(&state.db)
    .await
    .expect("Failed to get issuer's final balance");
    println!("After transfer: Issuer's balance = {}", issuer_final_balance.balance);
    assert_eq!(issuer_final_balance.balance, transfer_amount / 100); // 1% fee
}

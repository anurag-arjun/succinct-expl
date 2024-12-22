use ed25519_dalek::{Signer, SigningKey};
use rand::{Rng, RngCore, rngs::OsRng};
use sqlx::postgres::PgPoolOptions;
use std::{
    sync::{atomic::{AtomicI64, Ordering}, Arc},
    time::Instant,
};
use tokio::sync::broadcast;
use usda_common::WebSocketMessage;
use usda_core::state::AppState;

const NUM_USERS: usize = 10_000;  // Increased from 1000 to get more diversity in transfers
const NUM_TRANSFERS: usize = 1_000_000;  // Increased to 1 million
const MIN_TRANSFER: i64 = 1;
const MAX_TRANSFER: i64 = 1000;
const INITIAL_USER_BALANCE: i64 = 100_000;  // Increased to handle more transfers
const BATCH_SIZE: usize = 1000;  // Increased batch size for better throughput

async fn setup_test_state() -> Arc<AppState> {
    // Create a connection pool with more connections
    let pool = PgPoolOptions::new()
        .max_connections(50)  // Increased from 5 to 50 for better concurrency
        .connect("postgres://localhost/usda_test")
        .await
        .expect("Failed to create connection pool");

    // Clear existing data
    sqlx::query!("TRUNCATE accounts, transactions")
        .execute(&pool)
        .await
        .expect("Failed to truncate tables");

    // Create indexes for better performance
    sqlx::query!("CREATE INDEX IF NOT EXISTS idx_accounts_address ON accounts(address)")
        .execute(&pool)
        .await
        .expect("Failed to create account index");

    sqlx::query!("CREATE INDEX IF NOT EXISTS idx_transactions_from_addr ON transactions(from_addr)")
        .execute(&pool)
        .await
        .expect("Failed to create from_addr index");

    sqlx::query!("CREATE INDEX IF NOT EXISTS idx_transactions_to_addr ON transactions(to_addr)")
        .execute(&pool)
        .await
        .expect("Failed to create to_addr index");

    let (_tx, _) = broadcast::channel::<WebSocketMessage>(1000);
    Arc::new(AppState::new(pool))
}

async fn create_users(state: Arc<AppState>, count: usize) -> Vec<User> {
    let mut users = Vec::with_capacity(count);
    let mut total_queries = 0;
    let start = Instant::now();

    for _ in 0..count {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();
        let address = verifying_key.to_bytes();

        sqlx::query!(
            r#"
            INSERT INTO accounts (address, balance, nonce)
            VALUES ($1, $2, 0)
            "#,
            address.as_slice(),
            INITIAL_USER_BALANCE
        )
        .execute(&state.db)
        .await
        .expect("Failed to create user account");

        total_queries += 1;

        users.push(User {
            address,
            signing_key,
            nonce: Arc::new(AtomicI64::new(0)),
        });
    }

    let duration = start.elapsed();
    println!(
        "Account creation performance:\n\
         - Total accounts created: {}\n\
         - Total time: {:.2?}\n\
         - Average time per account: {:.2?}\n\
         - Accounts per second: {:.2}\n\
         - Total DB queries: {}",
        count,
        duration,
        duration / count as u32,
        count as f64 / duration.as_secs_f64(),
        total_queries
    );

    users
}

async fn create_fee_collector(state: Arc<AppState>) -> User {
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let address = verifying_key.to_bytes();

    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, nonce)
        VALUES ($1, $2, 0)
        "#,
        address.as_slice(),
        0_i64 // Start with 0 balance, will collect fees
    )
    .execute(&state.db)
    .await
    .expect("Failed to create fee collector account");

    User {
        address,
        signing_key,
        nonce: Arc::new(AtomicI64::new(0)),
    }
}

async fn get_user_balance(state: &AppState, address: &[u8]) -> i64 {
    sqlx::query!(
        r#"
        SELECT CAST(COALESCE(balance, 0) AS BIGINT) as balance
        FROM accounts
        WHERE address = $1
        "#,
        address
    )
    .fetch_one(&state.db)
    .await
    .unwrap()
    .balance
    .unwrap_or(0)
}

async fn get_total_balance(state: &AppState) -> i64 {
    // Get sum of all balances
    sqlx::query!(
        r#"
        SELECT CAST(COALESCE(SUM(balance), 0) AS BIGINT) as total
        FROM accounts
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap()
    .total
    .unwrap_or(0)
}

async fn get_total_pending(state: &AppState) -> i64 {
    // Get sum of all pending transactions
    sqlx::query!(
        r#"
        SELECT CAST(COALESCE(SUM(amount), 0) AS BIGINT) as total
        FROM transactions
        WHERE status = 'pending'
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap()
    .total
    .unwrap_or(0)
}

async fn get_total_fees(state: &AppState) -> i64 {
    // Get sum of all fees from confirmed transactions
    sqlx::query!(
        r#"
        SELECT CAST(COALESCE(SUM(fee), 0) AS BIGINT) as total
        FROM transactions
        WHERE status = 'confirmed'
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap()
    .total
    .unwrap_or(0)
}

async fn perform_random_transfers(
    state: Arc<AppState>,
    users: &[User],
    fee_collector: &User,
    num_transfers: usize,
) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut total_queries = 0;
    let start = Instant::now();
    let mut tx_ids = Vec::with_capacity(num_transfers);
    let progress_interval = num_transfers / 20; // Report progress every 5%

    // Create batches of transfers
    for batch_start in (0..num_transfers).step_by(BATCH_SIZE) {
        let batch_size = std::cmp::min(BATCH_SIZE, num_transfers - batch_start);
        let mut batch_futures = Vec::with_capacity(batch_size);

        // Prepare batch of transfers
        for _ in 0..batch_size {
            let from_idx = rng.gen_range(0..users.len());
            let mut to_idx = rng.gen_range(0..users.len());
            while to_idx == from_idx {
                to_idx = rng.gen_range(0..users.len());
            }

            let from_user = &users[from_idx];
            let to_user = &users[to_idx];
            let amount = rng.gen_range(MIN_TRANSFER..=MAX_TRANSFER);
            let fee = amount / 10; // 10% fee

            let nonce = from_user.nonce.fetch_add(1, Ordering::SeqCst);
            let message = format!(
                "{}{}{}{}{}",
                hex::encode(from_user.address),
                hex::encode(to_user.address),
                amount,
                fee,
                nonce
            );
            let signature = from_user.signing_key.sign(message.as_bytes());

            // Execute the query and collect the future
            let future = sqlx::query!(
                r#"
                WITH sender_check AS (
                    SELECT balance
                    FROM accounts
                    WHERE address = $1
                    FOR UPDATE SKIP LOCKED  -- Skip locked rows for better concurrency
                ),
                sender_update AS (
                    UPDATE accounts
                    SET balance = balance - $2,
                        nonce = nonce + 1
                    WHERE address = $1
                      AND EXISTS (
                          SELECT 1
                          FROM sender_check
                          WHERE balance >= $2
                      )
                    RETURNING address
                ),
                receiver_update AS (
                    INSERT INTO accounts (address, balance, nonce)
                    VALUES ($3, $4, 0)
                    ON CONFLICT (address) DO UPDATE
                    SET balance = accounts.balance + $4
                    WHERE EXISTS (SELECT 1 FROM sender_update)
                    RETURNING address
                ),
                fee_update AS (
                    UPDATE accounts
                    SET balance = balance + $5
                    WHERE address = $6
                      AND EXISTS (SELECT 1 FROM sender_update)
                    RETURNING address
                )
                INSERT INTO transactions (
                    tx_id, from_addr, to_addr, amount, fee,
                    nonce, signature, timestamp, status
                )
                SELECT $7, $1, $3, $4, $5, $8, $9, NOW(), 'PENDING'
                WHERE EXISTS (SELECT 1 FROM sender_update)
                  AND EXISTS (SELECT 1 FROM receiver_update)
                  AND EXISTS (SELECT 1 FROM fee_update)
                RETURNING tx_id
                "#,
                from_user.address.as_slice(),
                amount + fee,
                to_user.address.as_slice(),
                amount,
                fee,
                fee_collector.address.as_slice(),
                uuid::Uuid::new_v4().to_string(),
                nonce,
                signature.to_bytes().to_vec()
            )
            .fetch_one(&state.db);

            batch_futures.push(future);
            total_queries += 4; // 3 updates + 1 insert
        }

        // Execute batch concurrently
        let results = futures::future::join_all(batch_futures)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|r| r.tx_id);

        tx_ids.extend(results);

        // Report progress every 5%
        if (batch_start + batch_size) % progress_interval == 0 {
            let progress = (batch_start + batch_size) as f64 / num_transfers as f64 * 100.0;
            let elapsed = start.elapsed();
            let tps = (batch_start + batch_size) as f64 / elapsed.as_secs_f64();
            println!(
                "Progress: {:.1}% ({} transfers, {:.0} TPS)",
                progress,
                batch_start + batch_size,
                tps
            );
        }
    }

    let duration = start.elapsed();
    let total_tps = num_transfers as f64 / duration.as_secs_f64();
    println!(
        "\nTransfer performance:\n\
         - Total transfers: {}\n\
         - Total time: {:.2?}\n\
         - Average time per transfer: {:.2?}\n\
         - Overall TPS: {:.2}\n\
         - Total DB queries: {}\n\
         - Successful transfers: {}",
        num_transfers,
        duration,
        duration / num_transfers as u32,
        total_tps,
        total_queries,
        tx_ids.len()
    );

    tx_ids
}

#[tokio::test]
async fn benchmark_payment_system() {
    println!("\n=== Initializing Payment System Benchmark ===\n");

    // Step 1: Setup test state and create users
    let state = setup_test_state().await;
    let users = create_users(state.clone(), NUM_USERS).await;
    let fee_collector = create_fee_collector(state.clone()).await;

    // Log initial state
    let total_balance = get_total_balance(&state).await;
    let total_pending = get_total_pending(&state).await;
    let total_fees = get_total_fees(&state).await;

    println!("\n=== System State after Initial Setup ===");
    println!("Total balance: {}", total_balance);
    println!("Total pending: {}", total_pending);
    println!("Total fees: {}", total_fees);
    println!("====================\n");

    // Step 2: Perform random transfers between users
    println!("\n=== Performing Random Transfers ===");
    println!("Number of transfers: {}", NUM_TRANSFERS);
    println!("Min transfer amount: {}", MIN_TRANSFER);
    println!("Max transfer amount: {}", MAX_TRANSFER);

    perform_random_transfers(
        state.clone(),
        &users,
        &fee_collector,
        NUM_TRANSFERS,
    ).await;

    // Log final state
    let final_balance = get_total_balance(&state).await;
    let final_pending = get_total_pending(&state).await;
    let final_fees = get_total_fees(&state).await;

    println!("\n=== System State after Transfers ===");
    println!("Total balance: {}", final_balance);
    println!("Total pending: {}", final_pending);
    println!("Total fees: {}", final_fees);
    println!("====================\n");

    // Verify total balance remains constant
    assert_eq!(final_balance, INITIAL_USER_BALANCE * NUM_USERS as i64, "Total tokens should remain constant");
}

#[derive(Debug)]
struct User {
    address: [u8; 32],
    signing_key: SigningKey,
    nonce: Arc<AtomicI64>,
}

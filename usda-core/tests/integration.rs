use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use usda_common::TransactionStatus;
use usda_core::{api::*, state::AppState, websocket::WebSocketState};

async fn setup_test_app() -> (axum::Router, PgPool) {
    // Use a unique database for each test
    let db_name = format!("test_db_{}", uuid::Uuid::new_v4().simple());
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let admin_pool = PgPool::connect(&db_url).await.unwrap();

    // Create test database
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, db_name))
        .execute(&admin_pool)
        .await
        .unwrap();

    // Connect to test database
    let test_db_url = format!("{}/{}", db_url, db_name);
    let pool = PgPool::connect(&test_db_url).await.unwrap();

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    // Create app state
    let app_state = Arc::new(AppState::new(pool.clone()));
    let ws_state = Arc::new(WebSocketState::new(pool.clone()));

    // Build app
    let app = axum::Router::new()
        .route("/transfer", post(transfer))
        .route("/transactions/:tx_id", get(query::get_transaction_status))
        .route("/transactions", get(query::list_transactions))
        .route("/ws", get(websocket::handle_socket))
        .with_state(app_state)
        .with_state(ws_state);

    (app, pool)
}

#[tokio::test]
async fn test_transaction_lifecycle() {
    let (app, pool) = setup_test_app().await;

    // Create test account with balance
    let from_addr = [1u8; 32];
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, nonce, public_key)
        VALUES ($1, $2, $3, $4)
        "#,
        from_addr.as_slice(),
        1000i64,  // balance
        0i64,     // nonce
        [2u8; 32].as_slice()  // public key
    )
    .execute(&pool)
    .await
    .unwrap();

    // Submit transfer request
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/transfer")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "from": hex::encode(from_addr),
                        "to": hex::encode([3u8; 32]),
                        "amount": 100,
                        "fee": 10,
                        "nonce": 0,
                        "signature": hex::encode([4u8; 64])
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tx_id = response["tx_id"].as_str().unwrap();

    // Check initial transaction status
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/transactions/{}", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(status["status"], TransactionStatus::Executed.to_string());

    // Check balances
    let sender = sqlx::query!(
        r#"
        SELECT balance, nonce
        FROM accounts
        WHERE address = $1
        "#,
        from_addr.as_slice()
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(sender.balance, 890); // 1000 - 100 - 10
    assert_eq!(sender.nonce, 1);

    let receiver = sqlx::query!(
        r#"
        SELECT balance, nonce
        FROM accounts
        WHERE address = $1
        "#,
        [3u8; 32].as_slice()
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(receiver.balance, 100);
    assert_eq!(receiver.nonce, 0);
}

#[tokio::test]
async fn test_invalid_transfer() {
    let (app, _) = setup_test_app().await;

    // Submit transfer with invalid amount
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/transfer")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "from": hex::encode([1u8; 32]),
                        "to": hex::encode([2u8; 32]),
                        "amount": -100,  // Invalid amount
                        "fee": 10,
                        "nonce": 0,
                        "signature": hex::encode([3u8; 64])
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_insufficient_balance() {
    let (app, pool) = setup_test_app().await;

    // Create test account with small balance
    let from_addr = [1u8; 32];
    sqlx::query!(
        r#"
        INSERT INTO accounts (address, balance, nonce, public_key)
        VALUES ($1, $2, $3, $4)
        "#,
        from_addr.as_slice(),
        50i64,    // Small balance
        0i64,     // nonce
        [2u8; 32].as_slice()  // public key
    )
    .execute(&pool)
    .await
    .unwrap();

    // Submit transfer request with amount > balance
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/transfer")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "from": hex::encode(from_addr),
                        "to": hex::encode([3u8; 32]),
                        "amount": 100,  // More than balance
                        "fee": 10,
                        "nonce": 0,
                        "signature": hex::encode([4u8; 64])
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Check that balance hasn't changed
    let sender = sqlx::query!(
        r#"
        SELECT balance, nonce
        FROM accounts
        WHERE address = $1
        "#,
        from_addr.as_slice()
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(sender.balance, 50);
    assert_eq!(sender.nonce, 0);
}

use std::sync::Arc;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{AppError, AppState};
use usda_common::{WebSocketUpdate, TransactionUpdate, ProofUpdate, TransactionStatus, ProofStatus};

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Spawn task to forward messages from broadcast channel to websocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(_) => {
                    // Handle text messages if needed
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

pub async fn publish_transaction_update(
    state: &AppState,
    tx_id: Uuid,
    status: TransactionStatus,
    message: Option<String>,
) -> Result<(), AppError> {
    let update = WebSocketUpdate::Transaction(TransactionUpdate {
        tx_id: tx_id.to_string(),
        status,
        message,
    });

    state.tx.send(update)
        .map_err(|e| AppError::WebSocketError(e.to_string()))?;

    Ok(())
}

pub async fn publish_proof_update(
    state: &AppState,
    proof_id: Uuid,
    status: ProofStatus,
    message: Option<String>,
    num_transactions: i64,
) -> Result<(), AppError> {
    let update = WebSocketUpdate::Proof(ProofUpdate {
        proof_id: proof_id.to_string(),
        status,
        message,
        num_transactions,
    });

    state.tx.send(update)
        .map_err(|e| AppError::WebSocketError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_publish_transaction_update() {
        let (tx, mut rx) = broadcast::channel(100);
        let db = sqlx::PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let state = AppState::new(db, tx);

        let tx_id = Uuid::new_v4();
        let status = TransactionStatus::Processing;
        let message = Some("Test message".to_string());

        publish_transaction_update(&state, tx_id, status.clone(), message.clone())
            .await
            .unwrap();

        if let Ok(WebSocketUpdate::Transaction(update)) = rx.recv().await {
            assert_eq!(update.tx_id, tx_id.to_string());
            assert_eq!(update.status, status);
            assert_eq!(update.message, message);
        } else {
            panic!("Expected transaction update");
        }
    }

    #[tokio::test]
    async fn test_publish_proof_update() {
        let (tx, mut rx) = broadcast::channel(100);
        let db = sqlx::PgPool::connect("postgres://localhost/usda_test").await.unwrap();
        let state = AppState::new(db, tx);

        let proof_id = Uuid::new_v4();
        let status = ProofStatus::Processing;
        let message = Some("Test message".to_string());
        let num_transactions = 10;

        publish_proof_update(&state, proof_id, status.clone(), message.clone(), num_transactions)
            .await
            .unwrap();

        if let Ok(WebSocketUpdate::Proof(update)) = rx.recv().await {
            assert_eq!(update.proof_id, proof_id.to_string());
            assert_eq!(update.status, status);
            assert_eq!(update.message, message);
            assert_eq!(update.num_transactions, num_transactions);
        } else {
            panic!("Expected proof update");
        }
    }
}

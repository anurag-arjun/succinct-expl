use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{sync::broadcast, time};
use uuid::Uuid;
use usda_common::{WebSocketUpdate, TransactionUpdate, ProofUpdate};

use crate::AppState;

pub async fn handle_socket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket_connection(socket, state))
}

async fn handle_socket_connection(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.updates.subscribe();

    // Spawn task to forward updates to client
    let send_task = tokio::spawn(async move {
        let mut interval = time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if sender.send(axum::extract::ws::Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
                result = rx.recv() => {
                    match result {
                        Ok(msg) => {
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if sender.send(axum::extract::ws::Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    // Spawn task to handle client messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg {
                if let Ok(cmd) = serde_json::from_str::<SubscribeCommand>(&text) {
                    match cmd.command.as_str() {
                        "subscribe_transaction" => {
                            if let Some(tx_id) = cmd.tx_id {
                                // Handle transaction subscription
                                if let Ok(Some(tx)) = sqlx::query!(
                                    r#"
                                    SELECT status, error
                                    FROM transactions
                                    WHERE tx_id = $1
                                    "#,
                                    tx_id
                                )
                                .fetch_optional(&state.db)
                                .await
                                {
                                    let _ = state.updates.send(WebSocketUpdate::Transaction(TransactionUpdate {
                                        tx_id,
                                        status: tx.status,
                                        error: tx.error,
                                        timestamp: chrono::Utc::now(),
                                    }));
                                }
                            }
                        }
                        "subscribe_proof" => {
                            if let Some(batch_id) = cmd.batch_id {
                                // Handle proof subscription
                                if let Ok(Some(proof)) = sqlx::query!(
                                    r#"
                                    SELECT status, num_transactions, error
                                    FROM proofs
                                    WHERE batch_id = $1
                                    "#,
                                    batch_id
                                )
                                .fetch_optional(&state.db)
                                .await
                                {
                                    let _ = state.updates.send(WebSocketUpdate::Proof(ProofUpdate {
                                        batch_id,
                                        status: proof.status,
                                        num_transactions: proof.num_transactions,
                                        error: proof.error,
                                        timestamp: chrono::Utc::now(),
                                    }));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}

#[derive(Debug, Deserialize)]
pub struct SubscribeCommand {
    pub command: String,
    pub tx_id: Option<String>,
    pub batch_id: Option<Uuid>,
}

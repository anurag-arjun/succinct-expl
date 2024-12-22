use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use std::sync::Arc;

use crate::state::AppState;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
) {
    // Subscribe to broadcast channel
    let mut rx = state.ws_tx.subscribe();

    // Send messages as they come in
    while let Ok(msg) = rx.recv().await {
        if let Ok(json) = serde_json::to_string(&msg) {
            if socket
                .send(axum::extract::ws::Message::Text(json))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use tokio::io::{BufReader, AsyncBufReadExt};
use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum LightClientError {
    #[error("Process error: {0}")]
    ProcessError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockVerification {
    pub block_hash: String,
    pub block_number: u32,
    pub confidence: f64,
    pub cells_total: u32,
    pub cells_verified: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LightClientEvent {
    #[serde(rename = "block_verified")]
    BlockVerified(BlockVerification),
    #[serde(rename = "verification_progress")]
    VerificationProgress {
        block_hash: String,
        progress: f64,
        cells_verified: u32,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        block_hash: Option<String>,
    },
}

pub struct LightClient {
    process: Child,
    events_rx: tokio::sync::mpsc::Receiver<LightClientEvent>,
}

impl LightClient {
    pub async fn start(path: PathBuf, network: &str) -> Result<Self, LightClientError> {
        // Start the light client process with stdout piping
        let mut process = Command::new(path)
            .arg("--network")
            .arg(network)
            .arg("--log-format")
            .arg("json")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Create channel for events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Get stdout handle
        let stdout = process.stdout.take()
            .ok_or_else(|| LightClientError::ProcessError("Failed to capture stdout".to_string()))?;

        // Start output processing task
        let mut reader = BufReader::new(stdout).lines();
        let tx_clone = tx.clone();
        
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(event) = serde_json::from_str::<LightClientEvent>(&line) {
                    if tx_clone.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Also handle stderr
        let stderr = process.stderr.take()
            .ok_or_else(|| LightClientError::ProcessError("Failed to capture stderr".to_string()))?;

        let mut stderr_reader = BufReader::new(stderr).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                if let Ok(error) = serde_json::from_str::<LightClientEvent>(&line) {
                    if tx.send(error).await.is_err() {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            process,
            events_rx: rx,
        })
    }

    pub async fn next_event(&mut self) -> Option<LightClientEvent> {
        self.events_rx.recv().await
    }

    pub fn kill(&mut self) -> Result<(), LightClientError> {
        self.process.kill()?;
        Ok(())
    }
}

impl Drop for LightClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

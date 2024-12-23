mod light_client;

use sqlx::{PgPool, Row, migrate};
use thiserror::Error;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use light_client::{LightClient, LightClientEvent, LightClientError};

#[derive(Error, Debug)]
pub enum DASError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Light client error: {0}")]
    LightClientError(#[from] LightClientError),
    #[error("Verification error: {0}")]
    VerificationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "verification_status", rename_all = "lowercase")]
pub enum VerificationStatus {
    Pending,
    InProgress {
        progress: f64,
        cells_verified: u32,
    },
    Verified {
        confidence: f64,
        cells_total: u32,
    },
    Failed(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub id: Uuid,
    pub block_hash: String,
    pub block_number: i64,
    pub status: VerificationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct DASVerifier {
    pool: PgPool,
    light_client: Arc<RwLock<Option<LightClient>>>,
    light_client_path: PathBuf,
}

impl DASVerifier {
    pub async fn new(pool: PgPool, light_client_path: PathBuf) -> Result<Self, DASError> {
        // Ensure migrations are run
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| DASError::DatabaseError(e))?;

        let verifier = Self {
            pool,
            light_client: Arc::new(RwLock::new(None)),
            light_client_path,
        };

        // Start the light client and monitoring
        verifier.ensure_light_client_running().await?;

        Ok(verifier)
    }

    async fn ensure_light_client_running(&self) -> Result<(), DASError> {
        let mut light_client = self.light_client.write().await;
        if light_client.is_none() {
            *light_client = Some(
                LightClient::start(self.light_client_path.clone(), "goldberg").await?
            );
            
            // Start monitoring light client events
            self.start_event_monitoring();
        }
        Ok(())
    }

    fn start_event_monitoring(&self) {
        let light_client = self.light_client.clone();
        let pool = self.pool.clone();

        tokio::spawn(async move {
            while let Some(mut client) = light_client.write().await.as_mut() {
                while let Some(event) = client.next_event().await {
                    match event {
                        LightClientEvent::BlockVerified(verification) => {
                            let status = VerificationStatus::Verified {
                                confidence: verification.confidence,
                                cells_total: verification.cells_total,
                            };
                            
                            let _ = Self::update_verification_status_by_block(
                                &pool,
                                &verification.block_hash,
                                status
                            ).await;
                        },
                        LightClientEvent::VerificationProgress { block_hash, progress, cells_verified } => {
                            let status = VerificationStatus::InProgress {
                                progress,
                                cells_verified,
                            };
                            
                            let _ = Self::update_verification_status_by_block(
                                &pool,
                                &block_hash,
                                status
                            ).await;
                        },
                        LightClientEvent::Error { message, block_hash } => {
                            if let Some(hash) = block_hash {
                                let status = VerificationStatus::Failed(message);
                                let _ = Self::update_verification_status_by_block(
                                    &pool,
                                    &hash,
                                    status
                                ).await;
                            }
                        }
                    }
                }
                
                // If we get here, the light client has stopped sending events
                // Try to restart it
                *light_client.write().await = None;
            }
        });
    }

    async fn update_verification_status_by_block(
        pool: &PgPool,
        block_hash: &str,
        status: VerificationStatus,
    ) -> Result<(), DASError> {
        sqlx::query!(
            r#"
            UPDATE das_verifications
            SET status = $1::jsonb, updated_at = NOW()
            WHERE block_hash = $2
            "#,
            serde_json::to_value(&status).unwrap(),
            block_hash,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn start_verification(&self, block_hash: &str, block_number: i64) -> Result<Uuid, DASError> {
        // Ensure light client is running
        self.ensure_light_client_running().await?;

        let record = sqlx::query_as!(
            VerificationRecord,
            r#"
            INSERT INTO das_verifications (block_hash, block_number, status)
            VALUES ($1, $2, $3::jsonb)
            RETURNING id, block_hash, block_number, status as "status!: VerificationStatus", created_at, updated_at
            "#,
            block_hash,
            block_number,
            serde_json::to_value(&VerificationStatus::Pending).unwrap(),
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }

    pub async fn get_verification_status(&self, id: Uuid) -> Result<VerificationStatus, DASError> {
        let record = sqlx::query!(
            r#"
            SELECT status as "status!: VerificationStatus"
            FROM das_verifications
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.status)
    }

    pub async fn get_verification_progress(&self, id: Uuid) -> Result<(VerificationStatus, f64), DASError> {
        let record = sqlx::query!(
            r#"
            SELECT status as "status!: VerificationStatus"
            FROM das_verifications
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await?;

        let progress = match &record.status {
            VerificationStatus::Pending => 0.0,
            VerificationStatus::InProgress { progress, .. } => *progress,
            VerificationStatus::Verified { .. } => 1.0,
            VerificationStatus::Failed(_) => 1.0,
        };

        Ok((record.status, progress))
    }
}

impl Drop for DASVerifier {
    fn drop(&mut self) {
        // Light client will be cleaned up by its own Drop implementation
    }
}

use avail_subxt::{api, primitives::AppUncheckedExtrinsic};
use std::sync::Arc;
use subxt::OnlineClient;
use subxt_signer::sr25519::Keypair;
use thiserror::Error;
use std::path::PathBuf;
use sqlx::PgPool;
use crate::batch::{RollupBatch, BatchError};
use crate::finality::{FinalityTracker, FinalityConfig, FinalityError};
use crate::das::{DASVerifier, DASError, VerificationStatus};

#[derive(Error, Debug)]
pub enum AvailError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Submission error: {0}")]
    SubmissionError(String),
    #[error("Block error: {0}")]
    BlockError(String),
    #[error("Batch error: {0}")]
    BatchError(#[from] BatchError),
    #[error("Finality error: {0}")]
    FinalityError(#[from] FinalityError),
    #[error("DAS error: {0}")]
    DASError(#[from] DASError),
}

/// Configuration for Avail client
#[derive(Clone)]
pub struct AvailConfig {
    pub endpoint: String,
    pub keypair: Keypair,
    pub finality: FinalityConfig,
    pub light_client_path: PathBuf,
}

impl Default for AvailConfig {
    fn default() -> Self {
        Self {
            endpoint: "ws://127.0.0.1:9944".to_string(),
            keypair: Keypair::from_uri(&SecretUri::from_str("//Alice").unwrap()).unwrap(),
            finality: FinalityConfig::default(),
            light_client_path: PathBuf::from("/usr/local/bin/avail-light"),
        }
    }
}

/// Main client for interacting with Avail network
pub struct AvailClient {
    client: Arc<OnlineClient<api::AvailConfig>>,
    config: AvailConfig,
    finality_tracker: FinalityTracker,
    das_verifier: Arc<DASVerifier>,
}

impl AvailClient {
    /// Create a new Avail client instance
    pub async fn new(config: AvailConfig, pool: PgPool) -> Result<Self, AvailError> {
        let client = OnlineClient::from_url(&config.endpoint)
            .await
            .map_err(|e| AvailError::ConnectionError(e.to_string()))?;

        let finality_tracker = FinalityTracker::new(config.finality.clone());
        let das_verifier = Arc::new(
            DASVerifier::new(pool, config.light_client_path.clone())
                .await
                .map_err(AvailError::DASError)?
        );

        let client = Self {
            client: Arc::new(client),
            config,
            finality_tracker,
            das_verifier,
        };

        // Start monitoring new blocks
        client.start_block_monitoring();

        Ok(client)
    }

    /// Start monitoring new blocks for finality
    fn start_block_monitoring(&self) {
        let client = self.client.clone();
        let tracker = self.finality_tracker.clone();

        tokio::spawn(async move {
            let mut blocks = client.blocks().subscribe_finalized().await.unwrap();
            
            while let Some(block) = blocks.next().await {
                if let Ok(block) = block {
                    let number = block.header().number;
                    let hash = format!("{:?}", block.hash());
                    tracker.finalize_block(number, hash);
                }
            }
        });
    }

    /// Submit batch data to Avail and wait for verification
    pub async fn submit_batch_and_verify(&self, batch_data: Vec<u8>) -> Result<String, AvailError> {
        // Submit the batch
        let block_hash = self.submit_batch(batch_data).await?;
        
        // Get block details
        let block = self.get_block(block_hash.clone()).await?;
        let block_number = block.header().number as i64;

        // Start verification
        let verification_id = self.das_verifier
            .start_verification(&block_hash, block_number)
            .await
            .map_err(AvailError::DASError)?;

        // Wait for finality
        self.finality_tracker
            .wait_for_finality(&block_hash)
            .await?;

        // Check verification status
        match self.das_verifier.get_verification_status(verification_id).await? {
            VerificationStatus::Verified => Ok(block_hash),
            status => Err(AvailError::DASError(DASError::LightClientError(
                format!("Verification failed with status: {:?}", status)
            ))),
        }
    }

    /// Submit a rollup batch to Avail and wait for verification
    pub async fn submit_rollup_batch_and_verify(&self, batch: RollupBatch) -> Result<String, AvailError> {
        // Verify batch before submission
        batch.verify()?;
        
        // Encode batch
        let encoded_batch = batch.encode()?;
        
        // Submit and wait for verification
        self.submit_batch_and_verify(encoded_batch).await
    }

    /// Submit batch data to Avail and wait for verification with progress updates
    pub async fn submit_batch_and_verify_with_progress<F>(
        &self,
        batch_data: Vec<u8>,
        progress_callback: F,
    ) -> Result<String, AvailError>
    where
        F: Fn(f64) + Send + 'static,
    {
        // Submit the batch
        let block_hash = self.submit_batch(batch_data).await?;
        
        // Get block details
        let block = self.get_block(block_hash.clone()).await?;
        let block_number = block.header().number as i64;

        // Start verification
        let verification_id = self.das_verifier
            .start_verification(&block_hash, block_number)
            .await
            .map_err(AvailError::DASError)?;

        // Wait for finality first
        self.finality_tracker
            .wait_for_finality(&block_hash)
            .await?;

        // Monitor verification progress
        loop {
            let (status, progress) = self.das_verifier
                .get_verification_progress(verification_id)
                .await
                .map_err(AvailError::DASError)?;

            // Call progress callback
            progress_callback(progress);

            match status {
                VerificationStatus::Verified { .. } => {
                    return Ok(block_hash);
                }
                VerificationStatus::Failed(error) => {
                    return Err(AvailError::DASError(DASError::VerificationError(error)));
                }
                _ => {
                    // Continue waiting
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Submit a rollup batch to Avail and wait for verification with progress updates
    pub async fn submit_rollup_batch_and_verify_with_progress<F>(
        &self,
        batch: RollupBatch,
        progress_callback: F,
    ) -> Result<String, AvailError>
    where
        F: Fn(f64) + Send + 'static,
    {
        // Verify batch before submission
        batch.verify()?;
        
        // Encode batch
        let encoded_batch = batch.encode()?;
        
        // Submit and wait for verification
        self.submit_batch_and_verify_with_progress(encoded_batch, progress_callback).await
    }

    /// Submit batch data to Avail
    pub async fn submit_batch(&self, batch_data: Vec<u8>) -> Result<String, AvailError> {
        // Create the data submission extrinsic
        let tx = self
            .client
            .tx()
            .create_signed(
                &api::tx().data_availability().submit_data(batch_data),
                &self.config.keypair,
                Default::default(),
            )
            .await
            .map_err(|e| AvailError::SubmissionError(e.to_string()))?;

        // Submit and wait for inclusion
        let hash = tx
            .submit_and_watch()
            .await
            .map_err(|e| AvailError::SubmissionError(e.to_string()))?
            .wait_for_in_block()
            .await
            .map_err(|e| AvailError::SubmissionError(e.to_string()))?
            .block_hash();

        Ok(format!("{:?}", hash))
    }

    /// Get block details by hash
    pub async fn get_block(&self, block_hash: String) -> Result<api::Block, AvailError> {
        let hash = block_hash
            .parse()
            .map_err(|e| AvailError::BlockError(format!("Invalid block hash: {}", e)))?;

        self.client
            .blocks()
            .at(hash)
            .await
            .map_err(|e| AvailError::BlockError(e.to_string()))?
            .block()
            .ok_or_else(|| AvailError::BlockError("Block not found".to_string()))
    }

    /// Submit a rollup batch to Avail
    pub async fn submit_rollup_batch(&self, batch: RollupBatch) -> Result<String, AvailError> {
        // Verify batch before submission
        batch.verify()?;
        
        // Encode batch
        let encoded_batch = batch.encode()?;
        
        // Submit to Avail
        self.submit_batch(encoded_batch).await
    }

    /// Get rollup batch from Avail by block hash
    pub async fn get_rollup_batch(&self, block_hash: String) -> Result<RollupBatch, AvailError> {
        let block = self.get_block(block_hash).await?;
        
        // Extract batch data from block
        // Note: This is a simplified version - you'll need to implement proper data extraction
        // based on your specific extrinsic format
        let batch_data = block
            .extrinsics()
            .find(|ext| {
                // Add logic to identify your batch submission extrinsic
                true
            })
            .ok_or_else(|| AvailError::BlockError("Batch data not found in block".to_string()))?;

        // Decode batch
        RollupBatch::decode(batch_data.bytes())
            .map_err(AvailError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn test_finality_tracking() {
        // Create test config with short finality timeout
        let mut config = AvailConfig::default();
        config.finality.finality_timeout = Duration::from_secs(1);

        // Create client
        let client = AvailClient::new(config, PgPoolOptions::new().connect("postgres://localhost/testdb").await.unwrap()).await;
        assert!(client.is_ok());

        if let Ok(client) = client {
            // Create and submit test batch
            let batch = RollupBatch::new(
                [0u8; 32],
                100,
                110,
                vec![1, 2, 3],
                [1u8; 32],
            );

            // Submit and wait for finality
            let result = client.submit_rollup_batch_and_wait(batch).await;
            
            // This will fail without a running node, but should timeout cleanly
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), AvailError::FinalityError(FinalityError::Timeout)));
        }
    }

    #[tokio::test]
    async fn test_avail_client() {
        // Create test config
        let secret_uri = SecretUri::from_str("//Alice").unwrap();
        let keypair = Keypair::from_uri(&secret_uri).unwrap();
        
        let config = AvailConfig {
            endpoint: "ws://127.0.0.1:9944".to_string(),
            keypair,
            finality: FinalityConfig::default(),
            light_client_path: PathBuf::from("/usr/local/bin/avail-light"),
        };

        // Test client creation
        let client = AvailClient::new(config, PgPoolOptions::new().connect("postgres://localhost/testdb").await.unwrap()).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_rollup_batch_submission() {
        // Create test config
        let secret_uri = SecretUri::from_str("//Alice").unwrap();
        let keypair = Keypair::from_uri(&secret_uri).unwrap();
        
        let config = AvailConfig {
            endpoint: "ws://127.0.0.1:9944".to_string(),
            keypair,
            finality: FinalityConfig::default(),
            light_client_path: PathBuf::from("/usr/local/bin/avail-light"),
        };

        // Create test batch
        let batch = RollupBatch::new(
            [0u8; 32],
            100,
            110,
            vec![1, 2, 3],
            [1u8; 32],
        );

        // Test client creation and batch submission
        let client = AvailClient::new(config, PgPoolOptions::new().connect("postgres://localhost/testdb").await.unwrap()).await;
        assert!(client.is_ok());

        if let Ok(client) = client {
            let result = client.submit_rollup_batch(batch).await;
            // Note: This will fail without a running Avail node
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_das_verification() {
        // Setup database connection
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://localhost/testdb")
            .await
            .unwrap();

        // Create test config
        let mut config = AvailConfig::default();
        config.finality.finality_timeout = Duration::from_secs(1);
        config.light_client_path = PathBuf::from("/usr/local/bin/avail-light");

        // Create client
        let client = AvailClient::new(config, pool).await;
        assert!(client.is_ok());

        if let Ok(client) = client {
            // Create and submit test batch
            let batch = RollupBatch::new(
                [0u8; 32],
                100,
                110,
                vec![1, 2, 3],
                [1u8; 32],
            );

            // Submit and wait for verification
            let result = client.submit_rollup_batch_and_verify(batch).await;
            
            // This will fail without a running node and light client
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_das_verification_progress() {
        // Setup database connection
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://localhost/testdb")
            .await
            .unwrap();

        // Create test config
        let mut config = AvailConfig::default();
        config.finality.finality_timeout = Duration::from_secs(1);
        config.light_client_path = PathBuf::from("/usr/local/bin/avail-light");

        // Create client
        let client = AvailClient::new(config, pool).await;
        assert!(client.is_ok());

        if let Ok(client) = client {
            // Create test batch
            let batch = RollupBatch::new(
                [0u8; 32],
                100,
                110,
                vec![1, 2, 3],
                [1u8; 32],
            );

            // Track progress updates
            let progress_updates = Arc::new(std::sync::Mutex::new(Vec::new()));
            let progress_updates_clone = progress_updates.clone();

            // Submit and wait for verification
            let result = client
                .submit_rollup_batch_and_verify_with_progress(
                    batch,
                    move |progress| {
                        progress_updates_clone.lock().unwrap().push(progress);
                    },
                )
                .await;
            
            // This will fail without a running node and light client
            assert!(result.is_err());

            // But we should have received some progress updates
            let updates = progress_updates.lock().unwrap();
            assert!(!updates.is_empty());
            assert!(updates.contains(&0.0)); // Should have initial progress
        }
    }
}

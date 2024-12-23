use avail_subxt::api;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Error, Debug)]
pub enum FinalityError {
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    #[error("Subscription error: {0}")]
    SubscriptionError(String),
    #[error("Timeout waiting for finality")]
    Timeout,
}

/// Status of a block in the finality tracking system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStatus {
    /// Block is seen but not yet finalized
    Pending {
        number: u32,
        hash: String,
        timestamp: Instant,
    },
    /// Block is finalized
    Final {
        number: u32,
        hash: String,
        finalized_at: Instant,
    },
}

impl BlockStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, BlockStatus::Final { .. })
    }

    pub fn block_number(&self) -> u32 {
        match self {
            BlockStatus::Pending { number, .. } => *number,
            BlockStatus::Final { number, .. } => *number,
        }
    }

    pub fn block_hash(&self) -> &str {
        match self {
            BlockStatus::Pending { hash, .. } => hash,
            BlockStatus::Final { hash, .. } => hash,
        }
    }
}

/// Configuration for finality tracking
#[derive(Clone)]
pub struct FinalityConfig {
    /// Maximum time to wait for finality
    pub finality_timeout: Duration,
    /// Number of blocks required for finality
    pub finality_depth: u32,
    /// Maximum number of blocks to track
    pub max_tracked_blocks: usize,
}

impl Default for FinalityConfig {
    fn default() -> Self {
        Self {
            finality_timeout: Duration::from_secs(60),
            finality_depth: 20,
            max_tracked_blocks: 1000,
        }
    }
}

/// Tracks block finality status
#[derive(Clone)]
pub struct FinalityTracker {
    blocks: Arc<RwLock<HashMap<String, BlockStatus>>>,
    config: FinalityConfig,
    finality_tx: broadcast::Sender<BlockStatus>,
}

impl FinalityTracker {
    pub fn new(config: FinalityConfig) -> Self {
        let (finality_tx, _) = broadcast::channel(100);
        
        Self {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            config,
            finality_tx,
        }
    }

    /// Subscribe to finality updates
    pub fn subscribe(&self) -> broadcast::Receiver<BlockStatus> {
        self.finality_tx.subscribe()
    }

    /// Track a new block
    pub fn track_block(&self, number: u32, hash: String) {
        let mut blocks = self.blocks.write().unwrap();
        
        // Add new block
        blocks.insert(
            hash.clone(),
            BlockStatus::Pending {
                number,
                hash,
                timestamp: Instant::now(),
            },
        );

        // Cleanup old blocks
        if blocks.len() > self.config.max_tracked_blocks {
            let mut sorted: Vec<_> = blocks.iter().collect();
            sorted.sort_by_key(|(_, status)| status.block_number());
            
            let to_remove: Vec<_> = sorted
                .iter()
                .take(sorted.len() - self.config.max_tracked_blocks)
                .map(|(hash, _)| (*hash).clone())
                .collect();

            for hash in to_remove {
                blocks.remove(&hash);
            }
        }
    }

    /// Mark a block as finalized
    pub fn finalize_block(&self, number: u32, hash: String) {
        let mut blocks = self.blocks.write().unwrap();
        
        if let Some(status) = blocks.get_mut(&hash) {
            let new_status = BlockStatus::Final {
                number,
                hash: hash.clone(),
                finalized_at: Instant::now(),
            };
            *status = new_status.clone();
            
            // Notify subscribers
            let _ = self.finality_tx.send(new_status);
        }
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, hash: &str) -> bool {
        self.blocks
            .read()
            .unwrap()
            .get(hash)
            .map(|status| status.is_final())
            .unwrap_or(false)
    }

    /// Wait for a block to be finalized
    pub async fn wait_for_finality(&self, hash: &str) -> Result<BlockStatus, FinalityError> {
        let mut rx = self.subscribe();
        let start = Instant::now();

        // Check if already finalized
        if self.is_finalized(hash) {
            return self
                .blocks
                .read()
                .unwrap()
                .get(hash)
                .cloned()
                .ok_or_else(|| FinalityError::BlockNotFound(hash.to_string()));
        }

        // Wait for finalization
        while start.elapsed() < self.config.finality_timeout {
            match rx.recv().await {
                Ok(status) if status.block_hash() == hash && status.is_final() => {
                    return Ok(status);
                }
                Ok(_) => continue,
                Err(e) => return Err(FinalityError::SubscriptionError(e.to_string())),
            }
        }

        Err(FinalityError::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_finality_tracking() {
        let config = FinalityConfig {
            finality_timeout: Duration::from_secs(1),
            finality_depth: 2,
            max_tracked_blocks: 10,
        };
        
        let tracker = FinalityTracker::new(config);
        
        // Track a block
        let hash = "0x123".to_string();
        tracker.track_block(1, hash.clone());
        
        // Should not be finalized yet
        assert!(!tracker.is_finalized(&hash));
        
        // Finalize the block
        tracker.finalize_block(1, hash.clone());
        
        // Should be finalized now
        assert!(tracker.is_finalized(&hash));
        
        // Wait for finality should return immediately
        let status = tracker.wait_for_finality(&hash).await.unwrap();
        assert!(status.is_final());
    }

    #[tokio::test]
    async fn test_finality_timeout() {
        let config = FinalityConfig {
            finality_timeout: Duration::from_millis(100),
            finality_depth: 2,
            max_tracked_blocks: 10,
        };
        
        let tracker = FinalityTracker::new(config);
        
        // Wait for non-existent block should timeout
        let result = tracker.wait_for_finality("0x456").await;
        assert!(matches!(result, Err(FinalityError::Timeout)));
    }

    #[tokio::test]
    async fn test_block_cleanup() {
        let config = FinalityConfig {
            finality_timeout: Duration::from_secs(1),
            finality_depth: 2,
            max_tracked_blocks: 2,
        };
        
        let tracker = FinalityTracker::new(config);
        
        // Add 3 blocks
        tracker.track_block(1, "0x1".to_string());
        tracker.track_block(2, "0x2".to_string());
        tracker.track_block(3, "0x3".to_string());
        
        // Should only keep latest 2 blocks
        assert!(!tracker.is_finalized("0x1"));
        assert!(tracker.blocks.read().unwrap().len() == 2);
    }
}

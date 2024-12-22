use serde::{Serialize, Deserialize};

pub mod validation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProof {
    #[serde(with = "serde_arrays")]
    pub from_addr: [u8; 32],
    #[serde(with = "serde_arrays")]
    pub to_addr: [u8; 32],
    pub amount: i64,
    pub fee: i64,
    pub nonce: i64,
    #[serde(with = "serde_arrays")]
    pub signature: [u8; 64],
    #[serde(with = "serde_arrays")]
    pub public_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub cycles_used: u64,
}

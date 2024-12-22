use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_id: String,
    #[serde(with = "hex_array_opt")]
    pub from: Option<[u8; 32]>,
    #[serde(with = "hex_array")]
    pub to: [u8; 32],
    pub amount: i64,
    pub fee: i64,
    pub nonce: i64,
    #[serde(with = "hex_array")]
    pub signature: [u8; 64],
    pub timestamp: DateTime<Utc>,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Processing,
    Executed,
    Failed,
}

impl TransactionStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "processing" => Some(Self::Processing),
            "executed" => Some(Self::Executed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

impl fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Executed => write!(f, "executed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl TransactionStatus {
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            TransactionStatus::Executed | TransactionStatus::Failed
        )
    }

    pub fn is_error(&self) -> bool {
        matches!(self, TransactionStatus::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    #[serde(with = "hex_array")]
    pub address: [u8; 32],
    pub balance: i64,
    pub pending_balance: i64,
    pub nonce: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProof {
    pub batch_id: String,
    pub transactions: Vec<String>, // tx_ids
    pub proof_data: Vec<u8>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketMessage {
    TransactionPreconfirmed(Transaction),
    TransactionProven(Transaction),
    BalanceUpdated { 
        #[serde(with = "hex_array")]
        address: [u8; 32], 
        balance: i64 
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketUpdate {
    Transaction(TransactionUpdate),
    Proof(ProofUpdate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionUpdate {
    pub tx_id: String,
    pub status: TransactionStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofUpdate {
    pub proof_id: String,
    pub status: ProofStatus,
    pub message: Option<String>,
    pub num_transactions: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProofStatus {
    Pending,
    Processing,
    Generated,
    Failed,
}

mod hex_array {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let len = bytes.len();
        bytes.try_into().map_err(|_| {
            serde::de::Error::custom(format!(
                "Expected {} bytes but got {}",
                N,
                len
            ))
        })
    }
}

mod hex_array_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &Option<[u8; N]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => serializer.serialize_str(&hex::encode(bytes)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<Option<[u8; N]>, D::Error> {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => {
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                let bytes = bytes
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("Invalid byte array length"))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}

pub mod validation;

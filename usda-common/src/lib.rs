use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Proven,
    Failed,
}

impl fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "PENDING"),
            TransactionStatus::Proven => write!(f, "PROVEN"),
            TransactionStatus::Failed => write!(f, "FAILED"),
        }
    }
}

impl std::str::FromStr for TransactionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PENDING" => Ok(TransactionStatus::Pending),
            "PROVEN" => Ok(TransactionStatus::Proven),
            "FAILED" => Ok(TransactionStatus::Failed),
            _ => Err(format!("Invalid transaction status: {}", s)),
        }
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
pub enum WebSocketMessage {
    TransactionPreconfirmed(Transaction),
    TransactionProven(Transaction),
    BalanceUpdated { 
        #[serde(with = "hex_array")]
        address: [u8; 32], 
        balance: i64 
    },
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

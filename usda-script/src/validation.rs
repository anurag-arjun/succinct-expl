use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use sha2::{Sha256, Digest};
use thiserror::Error;

pub const MAX_BATCH_SIZE: usize = 100;
pub const MIN_AMOUNT: i64 = 0;
pub const MAX_AMOUNT: i64 = i64::MAX;
pub const MIN_FEE: i64 = 0;
pub const MAX_FEE: i64 = 1000000; // 1M units

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid amount: {0}")]
    InvalidAmount(i64),
    #[error("Invalid fee: {0}")]
    InvalidFee(i64),
    #[error("Invalid nonce: {0}")]
    InvalidNonce(i64),
    #[error("Invalid batch size: {0}")]
    InvalidBatchSize(usize),
    #[error("Insufficient balance: required {0}, available {1}")]
    InsufficientBalance(i64, i64),
}

impl From<ed25519_dalek::SignatureError> for ValidationError {
    fn from(_: ed25519_dalek::SignatureError) -> Self {
        ValidationError::InvalidSignature
    }
}

/// Validates a single transaction
pub fn validate_transaction(
    tx: &crate::TransferProof,
    current_nonce: i64,
    balance: i64,
) -> Result<(), ValidationError> {
    // Validate amount
    if tx.amount < MIN_AMOUNT || tx.amount > MAX_AMOUNT {
        return Err(ValidationError::InvalidAmount(tx.amount));
    }

    // Validate fee
    if tx.fee < MIN_FEE || tx.fee > MAX_FEE {
        return Err(ValidationError::InvalidFee(tx.fee));
    }

    // Validate nonce
    if tx.nonce != current_nonce + 1 {
        return Err(ValidationError::InvalidNonce(tx.nonce));
    }

    // Validate balance
    let total_required = tx.amount + tx.fee;
    if total_required > balance {
        return Err(ValidationError::InsufficientBalance(total_required, balance));
    }

    // Verify signature
    let msg = compute_message(tx);
    let signature = Signature::from_slice(&tx.signature)?;
    let public_key = VerifyingKey::from_bytes(&tx.public_key)?;
    
    public_key.verify(&msg, &signature)?;
    Ok(())
}

/// Validates a batch of transactions
pub fn validate_batch(
    txs: &[crate::TransferProof],
    initial_nonces: &[(Vec<u8>, i64)],
    initial_balances: &[(Vec<u8>, i64)],
) -> Result<(), ValidationError> {
    // Validate batch size
    if txs.len() > MAX_BATCH_SIZE {
        return Err(ValidationError::InvalidBatchSize(txs.len()));
    }

    // Track nonces and balances
    let mut nonces = initial_nonces.iter()
        .map(|(addr, nonce)| (addr.clone(), *nonce))
        .collect::<std::collections::HashMap<_, _>>();
    
    let mut balances = initial_balances.iter()
        .map(|(addr, balance)| (addr.clone(), *balance))
        .collect::<std::collections::HashMap<_, _>>();

    // Validate each transaction
    for tx in txs {
        let from_addr = tx.from_addr.to_vec();
        let to_addr = tx.to_addr.to_vec();
        
        // Get current nonce and balance
        let current_nonce = nonces.get(&from_addr).copied().unwrap_or(-1);
        let current_balance = balances.get(&from_addr).copied().unwrap_or(0);

        // Validate transaction
        validate_transaction(tx, current_nonce, current_balance)?;

        // Update nonce
        nonces.insert(from_addr.clone(), tx.nonce);

        // Update balances
        let new_from_balance = current_balance - tx.amount - tx.fee;
        balances.insert(from_addr, new_from_balance);

        let current_to_balance = balances.get(&to_addr).copied().unwrap_or(0);
        let new_to_balance = current_to_balance + tx.amount;
        balances.insert(to_addr, new_to_balance);
    }

    Ok(())
}

/// Computes the message to be signed
fn compute_message(tx: &crate::TransferProof) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(&tx.from_addr);
    hasher.update(&tx.to_addr);
    hasher.update(&tx.amount.to_le_bytes());
    hasher.update(&tx.fee.to_le_bytes());
    hasher.update(&tx.nonce.to_le_bytes());
    hasher.update(&tx.public_key);
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, Signer};

    fn create_signed_tx(
        from_addr: [u8; 32],
        to_addr: [u8; 32],
        amount: i64,
        fee: i64,
        nonce: i64,
        signing_key: &SigningKey,
    ) -> crate::TransferProof {
        let mut tx = crate::TransferProof {
            from_addr,
            to_addr,
            amount,
            fee,
            nonce,
            signature: [0u8; 64],
            public_key: signing_key.verifying_key().to_bytes(),
        };

        let msg = compute_message(&tx);
        let signature = signing_key.sign(&msg);
        tx.signature = signature.to_bytes();
        tx
    }

    #[test]
    fn test_valid_transaction() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let tx = create_signed_tx(
            from_addr,
            to_addr,
            100,
            10,
            1,
            &signing_key,
        );

        let result = validate_transaction(&tx, 0, 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_amount() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let tx = create_signed_tx(
            from_addr,
            to_addr,
            -1,
            10,
            1,
            &signing_key,
        );

        let result = validate_transaction(&tx, 0, 1000);
        assert!(matches!(result, Err(ValidationError::InvalidAmount(-1))));
    }

    #[test]
    fn test_invalid_fee() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let tx = create_signed_tx(
            from_addr,
            to_addr,
            100,
            MAX_FEE + 1,
            1,
            &signing_key,
        );

        let result = validate_transaction(&tx, 0, 1000);
        assert!(matches!(result, Err(ValidationError::InvalidFee(_))));
    }

    #[test]
    fn test_invalid_nonce() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let tx = create_signed_tx(
            from_addr,
            to_addr,
            100,
            10,
            2,  // Should be 1
            &signing_key,
        );

        let result = validate_transaction(&tx, 0, 1000);
        assert!(matches!(result, Err(ValidationError::InvalidNonce(2))));
    }

    #[test]
    fn test_insufficient_balance() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let tx = create_signed_tx(
            from_addr,
            to_addr,
            1000,
            10,
            1,
            &signing_key,
        );

        let result = validate_transaction(&tx, 0, 500);  // Only 500 available
        assert!(matches!(result, Err(ValidationError::InsufficientBalance(1010, 500))));
    }

    #[test]
    fn test_invalid_signature() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let mut tx = create_signed_tx(
            from_addr,
            to_addr,
            100,
            10,
            1,
            &signing_key,
        );

        // Tamper with the amount after signing
        tx.amount = 200;

        let result = validate_transaction(&tx, 0, 1000);
        assert!(matches!(result, Err(ValidationError::InvalidSignature)));
    }

    #[test]
    fn test_valid_batch() {
        let signing_key1 = SigningKey::from_bytes(&[1u8; 32]);
        let signing_key2 = SigningKey::from_bytes(&[2u8; 32]);
        let from_addr1 = [1u8; 32];
        let from_addr2 = [2u8; 32];
        let to_addr = [3u8; 32];

        let txs = vec![
            create_signed_tx(
                from_addr1,
                to_addr,
                100,
                10,
                1,
                &signing_key1,
            ),
            create_signed_tx(
                from_addr2,
                to_addr,
                200,
                20,
                1,
                &signing_key2,
            ),
        ];

        let initial_nonces = vec![
            (from_addr1.to_vec(), 0),
            (from_addr2.to_vec(), 0),
        ];

        let initial_balances = vec![
            (from_addr1.to_vec(), 1000),
            (from_addr2.to_vec(), 1000),
        ];

        let result = validate_batch(&txs, &initial_nonces, &initial_balances);
        assert!(result.is_ok());
    }

    #[test]
    fn test_batch_size_limit() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let from_addr = [1u8; 32];
        let to_addr = [2u8; 32];

        let txs = (0..MAX_BATCH_SIZE + 1)
            .map(|i| create_signed_tx(
                from_addr,
                to_addr,
                100,
                10,
                i as i64 + 1,
                &signing_key,
            ))
            .collect::<Vec<_>>();

        let initial_nonces = vec![(from_addr.to_vec(), 0)];
        let initial_balances = vec![(from_addr.to_vec(), 1000000)];

        let result = validate_batch(&txs, &initial_nonces, &initial_balances);
        assert!(matches!(result, Err(ValidationError::InvalidBatchSize(_))));
    }
}

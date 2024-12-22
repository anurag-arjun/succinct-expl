-- Add columns for batch processing
ALTER TABLE transactions
ADD COLUMN verified_at TIMESTAMP,
ADD COLUMN cycles_used BIGINT,
ADD COLUMN error TEXT;

-- Add public key column to accounts
ALTER TABLE accounts
ADD COLUMN public_key BYTEA;

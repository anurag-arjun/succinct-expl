-- Add proof tracking to transactions
ALTER TABLE transactions 
    ADD COLUMN proof_id UUID;

-- Add updated_at column to transactions if it doesn't exist
DO $$ 
BEGIN 
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                  WHERE table_name = 'transactions' 
                  AND column_name = 'updated_at') THEN
        ALTER TABLE transactions ADD COLUMN updated_at TIMESTAMPTZ DEFAULT NOW();
    END IF;
END $$;

-- Create proofs table for batch tracking
CREATE TABLE proofs (
    batch_id UUID PRIMARY KEY,
    proof_data BYTEA,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,
    status TEXT NOT NULL,
    num_transactions INTEGER NOT NULL,
    error TEXT
);

-- Add indexes for efficient querying
CREATE INDEX idx_transactions_proof_id ON transactions(proof_id);
CREATE INDEX idx_transactions_status_timestamp ON transactions(status, timestamp);

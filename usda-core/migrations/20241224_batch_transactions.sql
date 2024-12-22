-- Create batch_transactions table
CREATE TABLE IF NOT EXISTS batch_transactions (
    batch_id UUID NOT NULL REFERENCES proofs(batch_id),
    tx_id TEXT NOT NULL REFERENCES transactions(tx_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (batch_id, tx_id)
);

-- Create accounts table
CREATE TABLE accounts (
    address BYTEA PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0,
    pending_balance BIGINT NOT NULL DEFAULT 0,
    nonce BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create transactions table
CREATE TABLE transactions (
    tx_id TEXT PRIMARY KEY,
    from_addr BYTEA NOT NULL,
    to_addr BYTEA NOT NULL,
    amount BIGINT NOT NULL,
    fee BIGINT NOT NULL,
    nonce BIGINT NOT NULL,
    signature BYTEA NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    status TEXT NOT NULL,
    batch_id TEXT,
    FOREIGN KEY (from_addr) REFERENCES accounts(address),
    FOREIGN KEY (to_addr) REFERENCES accounts(address)
);

-- Create proof_batches table
CREATE TABLE proof_batches (
    batch_id TEXT PRIMARY KEY,
    proof_data BYTEA NOT NULL,
    transaction_count INTEGER NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    status TEXT NOT NULL
);

-- Create indexes
CREATE INDEX idx_transactions_from_addr ON transactions(from_addr);
CREATE INDEX idx_transactions_to_addr ON transactions(to_addr);
CREATE INDEX idx_transactions_timestamp ON transactions(timestamp);
CREATE INDEX idx_transactions_batch_id ON transactions(batch_id);

-- Create enum types
CREATE TYPE transaction_status AS ENUM ('PENDING', 'PROVEN', 'FAILED');
CREATE TYPE batch_status AS ENUM ('PROCESSING', 'COMPLETED', 'FAILED');

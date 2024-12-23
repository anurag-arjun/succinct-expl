-- Create enum for verification status
CREATE TYPE verification_status AS ENUM (
    'pending',
    'in_progress',
    'verified',
    'failed'
);

-- Create the das_verifications table
CREATE TABLE IF NOT EXISTS das_verifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    block_hash TEXT NOT NULL,
    block_number BIGINT NOT NULL,
    status JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index on block_hash for faster lookups
CREATE INDEX IF NOT EXISTS das_verifications_block_hash_idx ON das_verifications(block_hash);

-- Create index on status for filtering
CREATE INDEX IF NOT EXISTS das_verifications_status_idx ON das_verifications USING gin (status);

-- Add trigger to automatically update updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_das_verifications_updated_at
    BEFORE UPDATE ON das_verifications
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

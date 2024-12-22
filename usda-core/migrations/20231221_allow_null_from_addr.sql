-- Allow null from_addr for minting transactions
ALTER TABLE transactions DROP CONSTRAINT transactions_from_addr_fkey;
ALTER TABLE transactions ALTER COLUMN from_addr DROP NOT NULL;
ALTER TABLE transactions ADD CONSTRAINT transactions_from_addr_fkey 
    FOREIGN KEY (from_addr) REFERENCES accounts(address);

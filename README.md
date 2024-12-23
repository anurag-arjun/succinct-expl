# USDA (Unified Succinct Digital Assets)

A digital asset system that uses zero-knowledge proofs for transaction verification and Avail for data availability sampling, built on the SP1 ZK VM framework.

## Project Structure

The project consists of four main components:

- **usda-common**: Shared types, validation logic, and utilities
  - Transaction and proof status tracking
  - WebSocket update types and serialization
  - Account and balance management types
  - Error types and handling
  - Data Availability Sampling (DAS) verification
  - Avail client integration
  - Finality tracking

- **usda-core**: Core service implementation and API endpoints
  - Real-time WebSocket updates for transactions and proofs
  - Thread-safe state management with Arc and Mutex
  - Transaction processing with proper error handling
  - PostgreSQL integration with row-level locking
  - Broadcast channels for WebSocket messaging
  - Batch transaction processing
  - Worker pool for proof generation

- **usda-program**: Zero-knowledge proof program implementation
  - SP1 ZK VM integration
  - Transaction batch verification
  - State validation (balances, nonces)
  - Proof generation logic
  - Circuit optimization for batch processing

- **usda-script**: CLI tools and proof management
  - Proof generation and verification
  - Key management utilities
  - Transaction execution modes
  - Proving/verifying key management

## Development Status

### Completed Features
- [x] Basic account management with ED25519 keys
- [x] PostgreSQL integration with migrations
- [x] Transaction processing and validation
- [x] WebSocket real-time updates
- [x] Batch transaction processing
- [x] Data Availability Sampling integration
- [x] Avail client implementation
- [x] Light client process management
- [x] Verification progress tracking

### In Progress
- [ ] ZK proof generation optimization
- [ ] Batch verification circuits
- [ ] Performance benchmarking
- [ ] Monitoring and metrics
- [ ] Production deployment setup

### Planned Features
- [ ] Account recovery mechanisms
- [ ] Advanced transaction types
- [ ] Multi-signature support
- [ ] Cross-chain integration
- [ ] Advanced state sync mechanisms

## Features

### Core Features
- Account Management
  - Account creation with ED25519 key pairs
  - Balance and pending balance tracking
  - Transaction history with status updates
  - Real-time updates via WebSocket

### Transaction Processing
- Token transfers between accounts
- ED25519 signature verification
- Atomic balance updates with row-level locking
- Transaction status tracking (Pending, Processing, Executed, Failed)
- Batch processing for improved throughput
- Proper error handling for various failure cases:
  - Invalid input
  - Invalid amount
  - Invalid nonce
  - Invalid signature
  - Insufficient balance

### Data Availability Sampling (DAS)
- Integration with Avail's light client for DAS verification
- Real-time verification progress tracking
- Detailed verification state management:
  - Block verification status
  - Cell-level verification progress
  - Confidence metrics
- Automatic light client process management

### WebSocket Integration
- Real-time transaction status updates
- Proof generation progress updates
- DAS verification progress updates
- Structured message types for transactions, proofs, and verifications
- Broadcast channel for efficient message distribution
- Proper connection handling and cleanup

### API Endpoints
- `POST /transaction`: Create and process a new transaction
- `POST /batch`: Submit a batch of transactions
- `GET /transaction/:tx_id`: Get transaction status
- `GET /batch/:batch_id`: Get batch status
- `GET /ws`: WebSocket endpoint for real-time updates

## Getting Started

### Prerequisites
- Rust toolchain (2021 edition)
- PostgreSQL database
- SP1 ZK VM framework

### Database Setup
1. Set the database URL:
   ```bash
   export DATABASE_URL=postgres://localhost/usda_test
   ```

2. Run migrations:
   ```bash
   cargo sqlx migrate run
   ```

3. Create a PostgreSQL database:
   ```sql
   CREATE DATABASE usda_test;
   ```

4. Set up the schema:
   ```sql
   CREATE TABLE accounts (
     address BYTEA PRIMARY KEY,
     balance BIGINT NOT NULL,
     pending_balance BIGINT NOT NULL,
     nonce BIGINT NOT NULL,
     created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
   );

   CREATE TABLE transactions (
     tx_id UUID PRIMARY KEY,
     from_address BYTEA NOT NULL,
     to_address BYTEA NOT NULL,
     amount BIGINT NOT NULL,
     status TEXT NOT NULL,
     message TEXT,
     created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
   );

   CREATE TABLE das_verifications (
     id UUID PRIMARY KEY,
     block_hash TEXT NOT NULL,
     block_number BIGINT NOT NULL,
     status JSONB NOT NULL,
     created_at TIMESTAMPTZ NOT NULL,
     updated_at TIMESTAMPTZ NOT NULL
   );
   ```

### Installation
1. Clone the repository
2. Install dependencies: `cargo build`
3. Set environment variables:
   ```bash
   export DATABASE_URL=postgres://localhost/usda_test
   ```

### Running Tests
```bash
cargo test -p usda-common
cargo test -p usda-core
```

## Contributing
Contributions are welcome! Please feel free to submit a Pull Request.

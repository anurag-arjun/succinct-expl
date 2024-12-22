# USDA (Unified Succinct Digital Assets)

A digital asset system that uses zero-knowledge proofs for transaction verification.

## Project Structure

The project consists of three main components:

- **usda-common**: Shared types and utilities
- **usda-core**: Core service implementation and API endpoints
- **usda-program**: Zero-knowledge proof program implementation

## Features

### Implemented 

#### Account Management
- Account creation with ED25519 key pairs
- Balance retrieval
- Transaction history retrieval
- Real-time balance updates via WebSocket

#### Transaction Processing
- Token transfers between accounts
- Transaction signature verification
- Balance checks and updates
- Pending balance tracking
- Transaction fee handling (10%)
- Token minting (admin operation)
- Concurrent transaction processing with batching
- Row-level locking for consistent updates

#### Performance
- Sustained throughput of ~2,500 TPS in benchmarks
- Batch processing of 1,000 transactions per batch
- Optimized database queries with indexes
- Connection pooling with 50 concurrent connections

#### API Endpoints
- `POST /account/create`: Create a new account
- `GET /account/:address/balance`: Get account balance
- `GET /account/:address/transactions`: Get account transaction history
- `POST /transaction/transfer`: Transfer tokens between accounts
- `POST /transaction/mint`: Mint new tokens (admin only)
- `GET /ws`: WebSocket for real-time updates

#### Testing
- Unit tests for account operations
- Unit tests for transaction processing
- Database utilities for test setup
- Performance benchmarks for concurrent transfers
- Integration tests for API endpoints

### Pending 

#### Zero-Knowledge Proof Integration
- [ ] Implement proof generation in `usda-program`
- [ ] Add proof verification to transaction processing
- [ ] Update transaction status on proof verification
- [ ] Batch proof processing

#### Additional Features
- [ ] Batch transaction processing
- [ ] Rate limiting
- [ ] Admin dashboard
- [ ] Transaction fee configuration
- [ ] Account recovery mechanisms

#### Testing
- [ ] WebSocket notification tests
- [ ] Proof verification tests
- [ ] Security tests

#### Documentation
- [ ] API documentation
- [ ] Deployment guide
- [ ] Architecture documentation
- [ ] Development setup guide

#### Operations
- [ ] Metrics collection
- [ ] Structured logging
- [ ] Health check endpoints
- [ ] Database migration tools
- [ ] Monitoring dashboard
- [ ] Backup and recovery procedures

## Benchmarks

The system has been tested with the following workload:

### Account Creation
- 10,000 accounts created in 3.78s
- 2,643 accounts per second
- Each account initialized with 100,000 tokens

### Transaction Processing
- 1,000,000 transfers completed in 402.26s
- 2,486 transfers per second (sustained)
- 99.07% success rate (990,736 successful transfers)
- Consistent total balance maintained throughout
- Transfer amounts: 1-1,000 tokens
- 10% fee per transfer

### Database Performance
- 50 concurrent database connections
- Batch size of 1,000 transfers
- Row-level locking with SKIP LOCKED
- Indexes on frequently accessed columns
- 4 queries per transfer (sender, receiver, fee, transaction)

## Getting Started

### Prerequisites
- Rust 1.70 or later
- PostgreSQL 14 or later
- Docker (optional)

### Setup

1. Clone the repository:
```bash
git clone git@github.com:anurag-arjun/succinct-expl.git
cd succinct-expl
```

2. Set up the database:
```bash
# Set the database URL
export DATABASE_URL=postgres://localhost/usda_test

# Create the database
sqlx database create

# Run migrations
sqlx migrate run
```

3. Build and run tests:
```bash
# Build all components
cargo build

# Run unit tests
cargo test

# Run benchmark tests
cargo test --test benchmark_test -- --nocapture
```

4. Run the service:
```bash
cargo run
```

## License

[MIT License](LICENSE)

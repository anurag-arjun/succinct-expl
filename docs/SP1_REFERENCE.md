# SP1 Zero-Knowledge Proof Reference Guide

This document serves as a comprehensive reference for SP1 zero-knowledge proof development patterns and best practices.

## Table of Contents
1. [Program Structure](#program-structure)
2. [Input/Output Handling](#inputoutput-handling)
3. [Script Development](#script-development)
4. [Building and Running](#building-and-running)
5. [Best Practices](#best-practices)

## Program Structure

### Basic Program Template
```rust
#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Serialize, Deserialize};

// Your custom types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyStruct {
    // fields
}

pub fn main() {
    // Program logic
}
```

### Key Components
1. `#![no_main]` attribute is required
2. Use `sp1_zkvm::entrypoint!(main)` to declare the entrypoint
3. Keep the main function simple and modular
4. Use Serde for serialization/deserialization

## Input/Output Handling

### Reading Inputs
```rust
// Reading primitive types
let n = sp1_zkvm::io::read::<u32>();

// Reading custom types (must implement Serialize/Deserialize)
let my_struct = sp1_zkvm::io::read::<MyStruct>();
```

### Writing Outputs
```rust
// Committing outputs (makes them public to verifier)
let bytes = bincode::serialize(&result).unwrap();
sp1_zkvm::io::commit_slice(&bytes);
```

## Script Development

### Script Template
```rust
use clap::Parser;
use sp1_sdk::{SP1Prover, SP1Stdin, SP1Context};

#[derive(Parser, Debug)]
struct Args {
    /// Execute without proof generation
    #[arg(long)]
    execute: bool,
    
    /// Generate proof
    #[arg(long)]
    prove: bool,
}

fn main() {
    let args = Args::parse();
    
    // Setup inputs
    let mut stdin = SP1Stdin::new();
    stdin.write(&input);
    
    // Get program bytes
    let elf = include_bytes!(env!("SP1_ELF_program-name"));
    
    if args.execute {
        let mut prover = SP1Prover::new();
        let context = SP1Context::default();
        let (public_values, report) = prover.execute(elf, &stdin, context).unwrap();
        
        // Process outputs
        println!("Cycles used: {}", report.cycle_tracker.get("total").unwrap_or(&0));
    } else if args.prove {
        let mut prover = SP1Prover::new();
        let context = SP1Context::default();
        let program = prover.get_program(elf).unwrap();
        let proof = prover.prove_core(&program, &stdin, Default::default(), context).unwrap();
        println!("Proof generated successfully!");
    }
}
```

### Script Build Configuration
```rust
// build.rs
use sp1_helper::build_program_with_args;

fn main() {
    build_program_with_args("../program", Default::default());
}
```

## Building and Running

### Cargo Configuration
```toml
# program/Cargo.toml
[package]
name = "my-program"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-zkvm = "3.0.0-rc4"
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"

# script/Cargo.toml
[package]
name = "my-script"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-sdk = "3.0.0-rc4"
sp1-helper = "3.0.0-rc4"
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"

[build-dependencies]
sp1-helper = "3.0.0-rc4"
```

### Running
```bash
# Execute without proof
cargo run --release -- --execute

# Generate proof
cargo run --release -- --prove
```

## Best Practices

### Program Design
1. Keep programs modular and focused
2. Use appropriate data structures
3. Minimize memory operations
4. Use bincode for serialization

### Testing
1. Write comprehensive unit tests
2. Test both execution and proof generation
3. Verify input/output handling
4. Test edge cases

### Performance
1. Monitor cycle usage
2. Use batch processing where appropriate
3. Minimize memory operations
4. Profile and optimize critical paths

### Security
1. Validate all inputs
2. Don't expose sensitive data in public outputs
3. Use appropriate cryptographic primitives
4. Follow zero-knowledge best practices

use clap::Parser;
use sp1_sdk::{SP1Stdin, ProverClient};
use serde::{Serialize, Deserialize};
use bincode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProof {
    #[serde(with = "serde_arrays")]
    pub from_addr: [u8; 32],
    #[serde(with = "serde_arrays")]
    pub to_addr: [u8; 32],
    pub amount: i64,
    pub fee: i64,
    pub nonce: i64,
    #[serde(with = "serde_arrays")]
    pub signature: [u8; 64],
    #[serde(with = "serde_arrays")]
    pub public_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub cycles_used: u64,
}

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
    // Setup the logger
    sp1_sdk::utils::setup_logger();
    
    // Parse the command line arguments
    let args = Args::parse();
    
    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }
    
    // Setup test proofs
    let proofs = vec![
        TransferProof {
            from_addr: [1u8; 32],
            to_addr: [2u8; 32],
            amount: 100,
            fee: 10,
            nonce: 0,
            signature: [0u8; 64],
            public_key: [1u8; 32],
        },
        TransferProof {
            from_addr: [3u8; 32],
            to_addr: [4u8; 32],
            amount: 200,
            fee: 20,
            nonce: 1,
            signature: [0u8; 64],
            public_key: [3u8; 32],
        },
    ];
    
    // Setup the prover client
    let client = ProverClient::new();
    
    // Setup inputs
    let mut stdin = SP1Stdin::new();
    stdin.write(&(proofs.len() as u32));
    
    for proof in proofs {
        stdin.write(&proof);
    }
    
    let elf = include_bytes!(env!("SP1_ELF_usda-program"));
    
    if args.execute {
        // Execute the program
        let (output, report) = client.execute(elf, stdin).run().unwrap();
        println!("Program executed successfully.");
        
        // Read the output
        let result = bincode::deserialize::<BatchResult>(output.as_slice()).unwrap();
        println!("Result: {:?}", result);
        println!("Number of cycles: {}", report.total_instruction_count());
    } else if args.prove {
        // Setup the program for proving
        let (pk, vk) = client.setup(elf);
        
        // Generate the proof
        let proof = client.prove(&pk, stdin).run().unwrap();
        println!("Successfully generated proof!");
        
        // Verify the proof
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
        
        // TODO: Save proof to file
    }
}

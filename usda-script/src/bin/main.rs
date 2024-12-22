use clap::Parser;
use sp1_sdk::{SP1Stdin, ProverClient};
use serde::{Serialize, Deserialize};
use bincode;
use std::path::PathBuf;
use std::fs;

const PROVING_KEY_DIR: &str = "proving_keys";
const PROVING_KEY_FILE: &str = "usda_program.key";
const VERIFYING_KEY_FILE: &str = "usda_program.vk";

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

fn get_key_paths() -> (PathBuf, PathBuf) {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.push(PROVING_KEY_DIR);
    
    let mut pk_path = base_path.clone();
    pk_path.push(PROVING_KEY_FILE);
    
    let mut vk_path = base_path;
    vk_path.push(VERIFYING_KEY_FILE);
    
    (pk_path, vk_path)
}

fn ensure_proving_key_dir() -> std::io::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(PROVING_KEY_DIR);
    fs::create_dir_all(path)
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
        // Ensure proving key directory exists
        ensure_proving_key_dir().expect("Failed to create proving key directory");
        
        let (pk_path, vk_path) = get_key_paths();
        
        // Try to load existing proving and verifying keys
        let (pk, vk) = if pk_path.exists() && vk_path.exists() {
            println!("Loading existing proving and verifying keys...");
            let pk_bytes = fs::read(&pk_path).expect("Failed to read proving key");
            let vk_bytes = fs::read(&vk_path).expect("Failed to read verifying key");
            let pk = bincode::deserialize(&pk_bytes).expect("Failed to deserialize proving key");
            let vk = bincode::deserialize(&vk_bytes).expect("Failed to deserialize verifying key");
            (pk, vk)
        } else {
            println!("Generating new proving and verifying keys...");
            let (pk, vk) = client.setup(elf);
            // Save keys for future use
            let pk_bytes = bincode::serialize(&pk).expect("Failed to serialize proving key");
            let vk_bytes = bincode::serialize(&vk).expect("Failed to serialize verifying key");
            fs::write(&pk_path, pk_bytes).expect("Failed to write proving key");
            fs::write(&vk_path, vk_bytes).expect("Failed to write verifying key");
            (pk, vk)
        };
        
        println!("Generating proof...");
        let proof = client.prove(&pk, stdin).run().unwrap();
        println!("Successfully generated proof!");
        
        // Verify the proof
        client.verify(&proof, &vk).expect("Failed to verify proof");
        println!("Successfully verified proof!");
    }
}

//! solana-client-gen is a convenience macro to generate rpc clients from
//! Solana instruction definitions.
//!
//! # Creating an interface.
//!
//! To start, one should make an "interface" crate, separate from one's Solana
//! program, where instructions for the program are defined. For example,
//! one can have their interface in `my-crate/src/lib.rs`, while the program
//! lives in `my-crate/program/src/lib.rs`.
//!
//! // my-crate/src/lib.rs
//!
//! use solana_client_gen::prelude::*;
//!
//! #[cfg_attr(feature = "client", solana_client_gen)]
//! pub mod instruction {
//!   pub enum MyInstruction {
//!     Add: {
//!       a: u64,
//!       b: u64,
//!    },
//!    Substract: {
//!      a: u64,
//!      b: u64,
//!    },
//! }
//!
//! # Using a generated client.
//!
//! With this definition, one can include the crate, and use a generated RPC
//! client. Continuing the example above, one can invoke the `Add` instruction
//! on the Solana cluster:
//!
//! ```
//! use program_interface::client::{Client, ClientError};
//!
//! fn example() -> Result<Signature, ClientError> {
//!   // Client options.
//!   let program_id: Pubkey = "<your-program-id>>";
//!   let payer_filpath = "<your-id.json-path>";
//!   let cluster_url = "<your-closuter-url>>"
//!
//!   // Create the client.
//!   let client = SrmSafeClient::from_keypair_file(
//!     program_id,
//!     payer_filepath,
//!     cluster,
//!   )?
//!    .with_options(RequestOptions {
//!      commitment: CommitmentConfig::single(),
//!      tx: RpcSendTransactionConfig {
//!        skip_preflight: true,
//!        preflight_commitment: None,
//!      },
//!    });
//!
//!   Define the accounts the program uses for each instruction.
//!   let accounts = ...;
//!
//!   // Invoke rpcs as desired.
//!   client.add(accounts, 1, 2)?;
//!   client.sub(accounts, 4, 2)
//! ```
//!
//! In addition to generate an rpc client, the macro generates instruction
//! methods to create `solana_sdk::instruction::Instruction` types. For example,
//!
//! ```
//! let instruction = program_interface::instruction::add(1, 2);
//! ```
//!
//! This can be used to generate instructions to be invoked with other Solana
//! programs or to generate transactions manually,
//!
//! # Atomic account creation.
//!
//! It's not uncommon to want to atomically create an account and execute an
//! instruction. For example, in the SPL token standard, one must include
//! two instructions in the same transaction: one to create the mint account
//! and another to initialize the mint. To do this, add the #[create_account]
//! attribute to your instruction. For example
//!
//! ```
//! #[cfg_attr(feature = "client", solana_client_gen)]
//! pub mod instruction {
//!   #[cfg_attr(feature = "client", create_account)]
//!   Initialize {
//!     some_data: u64,
//!   }
//! }
//! ```
//! This will generate a `create_account_and_initialize` method. The api
//! works just like the others, except with one additional convention:
//! the *first* account in the `AccountInfo` array passed to the program
//! will always be the created account. Note: you don't have to pass the
//! account yourself, since it will be done for you.
//!
//! Furthermore, continuing the above example the simpler `initialize`
//! will also be generated.
//!
//! # Using a custom coder.
//!
//! By default, a default coder will be used to serialize instructions before
//! sending them to Solana. If you want to use a custom coder, inject it
//! into the macro like this
//!
//! ```
//! #[solana_client_gen(Coder)]
//! mod instruction {
//!   ...
//! }
//! ```
//!
//! Where `Coder` is a user defined struct that has two methods. For example,
//!
//! ```
//! struct Coder;
//! impl Coder {
//!   pub fn to_bytes(i: instruction::SrmSafeInstruction) -> Vec<u8> {
//!     println!("using custom coder");
//!      bincode::serialize(&(0u8, i)).expect("instruction must be serializable")
//!   }
//!   pub fn from_bytes(data: &[u8]) -> Result<instruction::SrmSafeInstruction, ()> {
//!     match data.split_first() {
//!       None => Err(()),
//!       Some((&u08, rest)) => bincode::deserialize(rest).map_err(|_| ()),
//!       Some((_, _rest)) => Err(()),
//!      }
//!    }
//! }
//! ```
//!
//! Note that the names must be `to_bytes` and `from_bytes` and the input/output of
//! both methods must be the name of your instruction's `enum`.
//!
//! # Limitations
//!
//! Currently, only a client is generated for serializing requests to Solana clusters.
//! This is why a conditional attribute macro is used (i.e., cfg_attr).
//!
//! In addition, it would be nice to generate code on the runtime as well to
//! deserialize instructions and dispatch to the correct method. Currently, this
//! isn't possible because the Solana Rust BPF toolchain uses Rust 1.39, which
//! doesn't support Rust's proc-macros. Once this is updated, we can generate
//! runtime/program code as well.
//!

// The prelude should be included by all crates using this macro.
pub mod prelude {
    pub use bincode;
    pub use serde;

    pub use solana_sdk::instruction::{AccountMeta, Instruction};
    pub use solana_sdk::pubkey::Pubkey;

    #[cfg(feature = "client")]
    pub use codegen::solana_client_gen;
    #[cfg(feature = "client")]
    pub use rand::rngs::OsRng;
    #[cfg(feature = "client")]
    pub use solana_client;
    #[cfg(feature = "client")]
    pub use solana_client::rpc_client::RpcClient;
    #[cfg(feature = "client")]
    pub use solana_client::rpc_config::RpcSendTransactionConfig;
    #[cfg(feature = "client")]
    pub use solana_sdk::commitment_config::CommitmentConfig;
    #[cfg(feature = "client")]
    pub use solana_sdk::signature::Keypair;
    #[cfg(feature = "client")]
    pub use solana_sdk::signature::{Signature, Signer};
    #[cfg(feature = "client")]
    pub use solana_sdk::signers::Signers;
    #[cfg(feature = "client")]
    pub use solana_sdk::system_instruction;
    #[cfg(feature = "client")]
    pub use solana_sdk::transaction::Transaction;
    #[cfg(feature = "client")]
    pub use thiserror::Error;
}

// Re-export.
pub use solana_sdk;

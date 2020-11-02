//! solana-client-gen is a convenience macro to generate rpc clients from
//! Solana instruction definitions.
//!
//! Ideally, Solana would have a Rust compiler extension built into their CLI,
//! which emitted a standard runtime IDL that could be used to generate clients.
//! This macro is a stopgap in the mean time.
//!
//! # Creating an interface.
//!
//! To start, one should make an "interface" crate, separate from one's Solana
//! program, where instructions for the program are defined. For example,
//! one can have their interface in `my-crate/src/lib.rs`, while the program
//! lives in `my-crate/program/src/lib.rs`.
//!
//!```
//! // my-crate/src/lib.rs
//!
//! use solana_client_gen::prelude::*;
//!
//! #[cfg_attr(feature = "client", solana_client_gen)]
//! pub mod instruction {
//!   #[derive(Serialize, Deserialize)]
//!   pub enum MyInstruction {
//!     Add {
//!       a: u64,
//!       b: u64,
//!    },
//!    Substract {
//!      a: u64,
//!      b: u64,
//!    },
//! }
//! ```
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
//!   let cluster_url = "<your-cluster-url>";
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
//!   // Define the accounts the program uses for each instruction.
//!   let accounts = ...;
//!
//!   // Invoke rpcs as desired.
//!   client.add(accounts, 1, 2)?;
//!   client.sub(accounts, 4, 2)
//! ```
//!
//! In addition to generating an rpc client, the macro generates instruction
//! methods to create `solana_sdk::instruction::Instruction` types. For example,
//!
//! ```
//! let instruction = my_crate::instruction::add(2, 3);
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
//! and another to initialize the mint. To do this, add the `#[create_account]`
//! attribute to your instruction. For example
//!
//! ```
//! #[cfg_attr(feature = "client", solana_client_gen)]
//! pub mod instruction {
//!   #[derive(Serialize, Deserialize)]
//!   pub enum MyInstruction {
//!     #[cfg_attr(feature = "client", create_account)]
//!     Initialize {
//!       some_data: u64,
//!     }
//!   }
//! }
//! ```
//! This will generate a `create_account_and_initialize` method. The api
//! works just like the others, except with one additional convention:
//! the *first* account in the `AccountInfo` array passed to the program
//! will always be the created account. Note: you don't have to pass the
//! account yourself, since it will be done for you.
//!
//! # Extending the client.
//!
//! If the generated client isn't enough, for example, if you want to batch
//! multiple instructions together into the same transaction as a performance
//! optimization, one can enable the macro's client-extension feature. This
//! Can be done by adding the "ext" argument, e.g., `#[solana_client_gen(ext)]`.
//!
//! When using the extension, the proc macro won't generate a client directly
//! Instead it will act as a meta-macro, generating yet another macro:
//! `solana_client_gen_extension`, which must be used to build the client.
//!
//! We can extend the above client to have a `hello_world` method.
//!
//! ```
//! solana_client_gen_extension! {
//!    impl Client {
//!      pub fn hello_world(&self) {
//!        println!("hello world from the generated client");
//!      }
//!    }
//! }
//! ```
//!
//! # Serialization
//!
//! Instructions used with this macro must implement the
//! `serum_common::pack::Pack` trait, where serialization should be defined.
//!
//! # Limitations
//!
//! Currently, only a client is generated for serializing requests to Solana clusters.
//! This is why a conditional attribute macro is used via `cfg_attr`.
//!
//! In addition, it would be nice to generate code on the runtime as well to
//! deserialize instructions and dispatch to the correct method. Currently, this
//! isn't possible because the Solana Rust BPF toolchain uses Rust 1.39, which
//! doesn't support Rust's proc-macros. Once this is updated, we can generate
//! runtime/program code as well.
//!

// The prelude should be included by all crates using this macro.
pub mod prelude {
    pub use solana_sdk;
    pub use solana_sdk::instruction::{AccountMeta, Instruction};

    pub use solana_sdk::pubkey::Pubkey;

    #[cfg(feature = "client")]
    pub use anyhow;
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

    #[cfg(feature = "client")]
    #[derive(Clone, Debug)]
    pub struct RequestOptions {
        pub commitment: CommitmentConfig,
        pub tx: RpcSendTransactionConfig,
    }

    #[cfg(feature = "client")]
    pub trait ClientGen: std::marker::Sized {
        fn from_keypair_file(program_id: Pubkey, filename: &str, url: &str)
            -> anyhow::Result<Self>;
        fn with_options(self, opts: RequestOptions) -> Self;
        fn rpc(&self) -> &RpcClient;
        fn payer(&self) -> &Keypair;
        fn program(&self) -> &Pubkey;
    }
}

// Re-export.
#[cfg(feature = "client")]
pub use solana_client;
pub use solana_sdk;

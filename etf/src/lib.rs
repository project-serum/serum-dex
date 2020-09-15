#![deny(missing_docs)]

//! Solana token-swap with the swap part ripped out, leaving something that's much like
//! an etf with creation/redepmtion capability

pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

// Export current solana-sdk types for downstream users who may also be building with a different
// solana-sdk version
pub use solana_sdk;

solana_sdk::declare_id!("TokenEtf11111111111111111111111111111111111");

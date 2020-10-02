//! The rewards module defines the rewards program interface the Registry
//! expects.

use solana_client_gen::solana_sdk::instruction::{AccountMeta, Instruction};
use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// Size of the data field of the "return_value" account for the rewards
/// program plugin.
pub const RETURN_VALUE_SIZE: usize = 8;

pub fn instruction(rewards_program_id: &Pubkey, accounts: &[AccountMeta]) -> Instruction {
    Instruction {
        program_id: *rewards_program_id,
        data: vec![0],
        accounts: accounts.to_vec(),
    }
}

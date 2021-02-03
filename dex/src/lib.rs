#![deny(safe_packed_borrows)]
#![allow(clippy::try_err)]

#[macro_use]
pub mod error;

#[cfg(test)]
mod tests;

pub mod critbit;
mod fees;
pub mod instruction;
pub mod matching;
pub mod state;

#[cfg(feature = "program")]
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, entrypoint, pubkey::Pubkey,
};

#[cfg(feature = "program")]
#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);
#[cfg(feature = "program")]
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    Ok(state::State::process(
        program_id,
        accounts,
        instruction_data,
    )?)
}

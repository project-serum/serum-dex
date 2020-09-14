//! Program entrypoint

mod api;
mod error;

use crate::error::{SafeError, SafeErrorCode};
use serum_safe_interface::instruction::SrmSafeInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;

// TODO: Once Solana updates their rust version, generate the entire decode
//       + dispatch step along with the Coder (or move the coder to the
//       interface crate if we want to manually serialize.

solana_sdk::entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Decode.
    let instruction: Result<SrmSafeInstruction, SafeError> = Coder::from_bytes(instruction_data)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::WrongSerialization));

    // Dispatch.
    match instruction? {
        SrmSafeInstruction::Initialize {
            admin_account_owner,
        } => api::initialize(accounts, admin_account_owner),
        SrmSafeInstruction::Slash { test } => api::slash(accounts),
        SrmSafeInstruction::DepositSrm {
            user_spl_wallet_owner,
            slot_number,
            amount,
            lsrm_amount,
        } => api::deposit_srm(
            accounts,
            user_spl_wallet_owner,
            slot_number,
            amount,
            lsrm_amount,
        ),
        SrmSafeInstruction::WithdrawSrm { amount } => api::withdraw_srm(accounts, amount),
        SrmSafeInstruction::MintLockedSrm { amount } => api::mint_locked_srm(accounts, amount),
        SrmSafeInstruction::BurnLockedSrm { amount } => api::burn_locked_srm(accounts, amount),
    };

    Ok(())
}

// Coder is the instruction deserializer.
pub struct Coder;
impl Coder {
    pub fn from_bytes(data: &[u8]) -> Result<SrmSafeInstruction, ()> {
        match data.split_first() {
            None => Err(()),
            Some((&u08, rest)) => bincode::deserialize(rest).map_err(|_| ()),
            Some((_, _rest)) => Err(()),
        }
    }
}

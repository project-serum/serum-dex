//! Program entrypoint

mod api;

use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::instruction::SrmSafeInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
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
    info!("INSTRUCTION ENTER");
    // Decode.
    let instruction: SrmSafeInstruction = Coder::from_bytes(instruction_data)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::WrongSerialization))?;

    // Dispatch.
    let result = match instruction {
        SrmSafeInstruction::Initialize { mint, authority } => {
            api::initialize(program_id, accounts, mint, authority)
        }
        SrmSafeInstruction::Slash { amount } => api::slash(accounts, amount),
        SrmSafeInstruction::DepositSrm {
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        } => api::deposit_srm(
            program_id,
            accounts,
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        ),
        SrmSafeInstruction::WithdrawSrm { amount } => api::withdraw_srm(accounts, amount),
        SrmSafeInstruction::MintLockedSrm { amount } => api::mint_locked_srm(accounts, amount),
        SrmSafeInstruction::BurnLockedSrm { amount } => api::burn_locked_srm(accounts, amount),
    };

    result?;

    info!("INSTRUCTION SUCCESS");

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

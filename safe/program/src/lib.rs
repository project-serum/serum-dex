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
        SrmSafeInstruction::Initialize {
            mint,
            authority,
            nonce,
        } => api::initialize(program_id, accounts, mint, authority, nonce),
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
        SrmSafeInstruction::MintLockedSrm => api::mint_locked_srm(program_id, accounts),
        SrmSafeInstruction::WithdrawSrm { amount } => {
            api::withdraw_srm(program_id, accounts, amount)
        }
        SrmSafeInstruction::BurnLockedSrm => api::burn_locked_srm(accounts),
        SrmSafeInstruction::Slash { amount } => api::slash(accounts, amount),
        SrmSafeInstruction::WhitelistAdd { program_id_to_add } => {
            api::whitelist_add(program_id, accounts, program_id_to_add)
        }
        SrmSafeInstruction::WhitelistDelete {
            program_id_to_delete,
        } => api::whitelist_delete(program_id, accounts, program_id_to_delete),
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

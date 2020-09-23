//! Program entrypoint

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::instruction::SafeInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

mod burn;
mod deposit;
mod initialize;
mod migrate;
mod mint;
mod set_authority;
mod withdraw;

solana_sdk::entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: SafeInstruction = serum_common::pack::from_bytes(instruction_data)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::WrongSerialization))?;

    let result = match instruction {
        SafeInstruction::Initialize { authority, nonce } => {
            initialize::handler(program_id, accounts, authority, nonce)
        }
        SafeInstruction::Deposit {
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        } => deposit::handler(
            program_id,
            accounts,
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        ),
        SafeInstruction::MintLocked {
            token_account_owner,
        } => mint::handler(program_id, accounts, token_account_owner),
        SafeInstruction::Withdraw { amount } => withdraw::handler(program_id, accounts, amount),
        SafeInstruction::BurnLocked => burn::handler(program_id, accounts),
        SafeInstruction::SetAuthority { new_authority } => {
            set_authority::handler(program_id, accounts, new_authority)
        }
        SafeInstruction::Migrate => migrate::handler(program_id, accounts),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::instruction::SafeInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

mod claim;
mod deposit;
mod initialize;
mod migrate;
mod redeem;
mod set_authority;
mod whitelist_add;
mod whitelist_delete;
mod whitelist_deposit;
mod whitelist_withdraw;

solana_sdk::entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: SafeInstruction = SafeInstruction::unpack(instruction_data)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::WrongSerialization))?;

    let result = match instruction {
        SafeInstruction::Initialize { authority, nonce } => {
            initialize::handler(program_id, accounts, authority, nonce)
        }
        SafeInstruction::Deposit {
            beneficiary,
            end_slot,
            period_count,
            deposit_amount,
        } => deposit::handler(
            program_id,
            accounts,
            beneficiary,
            end_slot,
            period_count,
            deposit_amount,
        ),
        SafeInstruction::Claim => claim::handler(program_id, accounts),
        SafeInstruction::Redeem { amount } => redeem::handler(program_id, accounts, amount),
        SafeInstruction::WhitelistWithdraw {
            amount,
            instruction_data,
        } => whitelist_withdraw::handler(program_id, accounts, amount, instruction_data),
        SafeInstruction::WhitelistDeposit { instruction_data } => {
            whitelist_deposit::handler(program_id, accounts, instruction_data)
        }
        SafeInstruction::WhitelistAdd { program_id_to_add } => {
            whitelist_add::handler(program_id, accounts, program_id_to_add)
        }
        SafeInstruction::WhitelistDelete {
            program_id_to_delete,
        } => whitelist_delete::handler(program_id, accounts, program_id_to_delete),
        SafeInstruction::SetAuthority { new_authority } => {
            set_authority::handler(program_id, accounts, new_authority)
        }
        SafeInstruction::Migrate => migrate::handler(program_id, accounts),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

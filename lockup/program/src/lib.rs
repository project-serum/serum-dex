#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_lockup::error::{LockupError, LockupErrorCode};
use serum_lockup::instruction::LockupInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::pubkey::Pubkey;

mod available_for_withdrawal;
mod common;
mod create_vesting;
mod initialize;
mod set_authority;
mod whitelist_add;
mod whitelist_delete;
mod whitelist_deposit;
mod whitelist_withdraw;
mod withdraw;

solana_program::entrypoint!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let instruction: LockupInstruction = LockupInstruction::unpack(instruction_data)
        .map_err(|_| LockupError::ErrorCode(LockupErrorCode::WrongSerialization))?;

    let result = match instruction {
        LockupInstruction::Initialize { authority } => {
            initialize::handler(program_id, accounts, authority)
        }
        LockupInstruction::CreateVesting {
            beneficiary,
            end_ts,
            period_count,
            deposit_amount,
            nonce,
        } => create_vesting::handler(
            program_id,
            accounts,
            beneficiary,
            end_ts,
            period_count,
            deposit_amount,
            nonce,
        ),
        LockupInstruction::Withdraw { amount } => withdraw::handler(program_id, accounts, amount),
        LockupInstruction::WhitelistWithdraw {
            amount,
            instruction_data,
        } => whitelist_withdraw::handler(program_id, accounts, amount, instruction_data),
        LockupInstruction::WhitelistDeposit { instruction_data } => {
            whitelist_deposit::handler(program_id, accounts, instruction_data)
        }
        LockupInstruction::WhitelistAdd { entry } => {
            whitelist_add::handler(program_id, accounts, entry)
        }
        LockupInstruction::WhitelistDelete { entry } => {
            whitelist_delete::handler(program_id, accounts, entry)
        }
        LockupInstruction::SetAuthority { new_authority } => {
            set_authority::handler(program_id, accounts, new_authority)
        }
        LockupInstruction::AvailableForWithdrawal => {
            available_for_withdrawal::handler(program_id, accounts)
        }
    };

    result?;

    Ok(())
}

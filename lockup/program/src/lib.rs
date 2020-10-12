//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_lockup::error::{LockupError, LockupErrorCode};
use serum_lockup::instruction::LockupInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub(crate) mod access_control;
mod claim;
mod create_vesting;
mod initialize;
mod migrate;
mod redeem;
mod set_authority;
mod whitelist_add;
mod whitelist_delete;
mod whitelist_deposit;
mod whitelist_withdraw;

solana_sdk::entrypoint!(entry);
fn entry<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: LockupInstruction = LockupInstruction::unpack(instruction_data)
        .map_err(|_| LockupError::ErrorCode(LockupErrorCode::WrongSerialization))?;

    let result = match instruction {
        LockupInstruction::Initialize { authority, nonce } => {
            initialize::handler(program_id, accounts, authority, nonce)
        }
        LockupInstruction::CreateVesting {
            beneficiary,
            end_ts,
            period_count,
            deposit_amount,
        } => create_vesting::handler(
            program_id,
            accounts,
            beneficiary,
            end_ts,
            period_count,
            deposit_amount,
        ),
        LockupInstruction::Claim => claim::handler(program_id, accounts),
        LockupInstruction::Redeem { amount } => redeem::handler(program_id, accounts, amount),
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
        LockupInstruction::Migrate => migrate::handler(program_id, accounts),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

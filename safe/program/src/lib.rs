//! Program entrypoint

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::instruction::SrmSafeInstruction;
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
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("INSTRUCTION ENTER");
    let instruction: SrmSafeInstruction = coder::from_bytes(instruction_data)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::WrongSerialization))?;

    let result = match instruction {
        SrmSafeInstruction::Initialize {
            mint,
            authority,
            nonce,
        } => initialize::handler(program_id, accounts, mint, authority, nonce),
        SrmSafeInstruction::DepositSrm {
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
        SrmSafeInstruction::MintLockedSrm => mint::handler(program_id, accounts),
        SrmSafeInstruction::WithdrawSrm { amount } => {
            withdraw::handler(program_id, accounts, amount)
        }
        SrmSafeInstruction::BurnLockedSrm => burn::handler(program_id, accounts),
        SrmSafeInstruction::SetAuthority { new_authority } => {
            set_authority::handler(program_id, accounts, &new_authority)
        }
        SrmSafeInstruction::Migrate => migrate::handler(program_id, accounts),
    };

    result?;

    info!("INSTRUCTION SUCCESS");

    Ok(())
}

mod coder {
    use super::SrmSafeInstruction;

    pub fn from_bytes(data: &[u8]) -> Result<SrmSafeInstruction, ()> {
        match data.split_first() {
            None => Err(()),
            Some((&0u8, rest)) => bincode::deserialize(rest).map_err(|_| ()),
            Some((_, _rest)) => Err(()),
        }
    }
}

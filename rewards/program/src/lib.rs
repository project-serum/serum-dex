#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_rewards::error::{RewardsError, RewardsErrorCode};
use serum_rewards::instruction::RewardsInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub(crate) mod access_control;
mod crank_relay;
mod initialize;
mod migrate;
mod set_authority;

solana_sdk::entrypoint!(entry);
fn entry<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: RewardsInstruction = RewardsInstruction::unpack(instruction_data)
        .map_err(|_| RewardsError::ErrorCode(RewardsErrorCode::WrongSerialization))?;

    let result = match instruction {
        RewardsInstruction::Initialize {
            nonce,
            registry_program_id,
            dex_program_id,
            authority,
        } => initialize::handler(
            program_id,
            accounts,
            nonce,
            registry_program_id,
            dex_program_id,
            authority,
        ),
        RewardsInstruction::CrankRelay { instruction_data } => {
            crank_relay::handler(program_id, accounts, instruction_data)
        }
        RewardsInstruction::SetAuthority { authority } => {
            set_authority::handler(program_id, accounts, authority)
        }
        RewardsInstruction::Migrate => migrate::handler(program_id, accounts),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

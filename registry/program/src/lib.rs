//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use serum_registry::instruction::RegistryInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

mod create_entity;
mod donate;
mod end_stake_withdrawal;
mod initialize;
mod join_entity;
mod register_capability;
mod stake;
mod start_stake_withdrawal;
mod update_entity;

solana_sdk::entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: RegistryInstruction = RegistryInstruction::unpack(instruction_data)
        .map_err(|_| RegistryError::ErrorCode(RegistryErrorCode::WrongSerialization))?;

    let result = match instruction {
        RegistryInstruction::Initialize {
            authority,
            withdrawal_timelock,
        } => initialize::handler(program_id, accounts, authority, withdrawal_timelock),
        RegistryInstruction::RegisterCapability {
            capability_id,
            capability_fee_bps,
        } => register_capability::handler(program_id, accounts, capability_id, capability_fee_bps),
        RegistryInstruction::CreateEntity {
            capabilities,
            stake_kind,
        } => create_entity::handler(program_id, accounts, capabilities, stake_kind),
        RegistryInstruction::UpdateEntity {
            leader,
            capabilities,
        } => update_entity::handler(program_id, accounts, leader, capabilities),
        RegistryInstruction::JoinEntity {
            beneficiary,
            delegate,
        } => join_entity::handler(program_id, accounts, beneficiary, delegate),
        RegistryInstruction::Stake { amount, is_mega } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::StartStakeWithdrawal {
            amount,
            mega_amount,
        } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::EndStakeWithdrawal => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::Donate { amount } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use serum_registry::instruction::RegistryInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

mod collect_rewards;
mod complete_stake_withdrawal;
mod create_entity;
mod donate;
mod initialize;
mod initiate_stake_withdrawal;
mod register_capability;
mod set_rewards;
mod stake;
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
        RegistryInstruction::Initialize { authority, nonce } => {
            initialize::handler(program_id, accounts, authority, nonce)
        }
        RegistryInstruction::SetRewards {
            rewards,
            rewards_return_value,
        } => set_rewards::handler(program_id, accounts, rewards, rewards_return_value),
        RegistryInstruction::Donate { amount } => donate::handler(program_id, accounts, amount),
        RegistryInstruction::CreateEntity {
            capabilities,
            stake_kind,
        } => create_entity::handler(program_id, accounts, capabilities, stake_kind),
        RegistryInstruction::UpdateEntity { capabilities } => {
            update_entity::handler(program_id, accounts, capabilities)
        }
        RegistryInstruction::RegisterCapability {
            capability_id,
            capability_program,
        } => register_capability::handler(program_id, accounts, capability_id, capability_program),
        RegistryInstruction::Stake {
            amount,
            beneficiary,
            is_mega,
        } => stake::handler(program_id, accounts, amount, beneficiary, is_mega),
        RegistryInstruction::CollectRewards => collect_rewards::handler(program_id, accounts),
        RegistryInstruction::AddStake { amount } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::StakeLocked {
            amount,
            beneficiary,
        } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::InitiateStakeWithdrawal {
            amount,
            mega_amount,
        } => Err(RegistryError::ErrorCode(
            RegistryErrorCode::NotReadySeeNextMajorVersion,
        )),
        RegistryInstruction::CompleteStakeWithdrawal { is_token, is_mega } => Err(
            RegistryError::ErrorCode(RegistryErrorCode::NotReadySeeNextMajorVersion),
        ),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

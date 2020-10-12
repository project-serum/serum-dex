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
mod stake_intent;
mod stake_intent_withdrawal;
mod start_stake_withdrawal;
mod transfer_stake_intent;
mod update_entity;
mod update_member;

solana_sdk::entrypoint!(entry);
fn entry<'a>(
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
            nonce,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        } => initialize::handler(
            program_id,
            accounts,
            authority,
            nonce,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        ),
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
            watchtower,
        } => join_entity::handler(program_id, accounts, beneficiary, delegate, watchtower),
        RegistryInstruction::UpdateMember {
            watchtower,
            delegate,
        } => update_member::handler(program_id, accounts, watchtower, delegate),
        RegistryInstruction::StakeIntent {
            amount,
            mega,
            delegate,
        } => stake_intent::handler(program_id, accounts, amount, mega, delegate),
        RegistryInstruction::StakeIntentWithdrawal {
            amount,
            mega,
            delegate,
        } => stake_intent_withdrawal::handler(program_id, accounts, amount, mega, delegate),
        RegistryInstruction::Stake {
            amount,
            mega,
            delegate,
        } => stake::handler(program_id, accounts, amount, mega, delegate),
        RegistryInstruction::StartStakeWithdrawal {
            amount,
            mega,
            delegate,
        } => start_stake_withdrawal::handler(program_id, accounts, amount, mega, delegate),
        RegistryInstruction::TransferStakeIntent {
            amount,
            mega,
            delegate,
        } => transfer_stake_intent::handler(program_id, accounts, amount, mega, delegate),
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

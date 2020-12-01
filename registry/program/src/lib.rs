#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use serum_registry::instruction::RegistryInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::pubkey::Pubkey;

mod claim_locked_reward;
mod common;
mod create_entity;
mod create_member;
mod deposit;
mod drop_locked_reward;
mod drop_pool_reward;
mod end_stake_withdrawal;
mod initialize;
mod mark_generation;
mod slash;
mod stake;
mod start_stake_withdrawal;
mod switch_entity;
mod update_entity;
mod update_member;
mod update_registrar;
mod withdraw;

solana_program::entrypoint!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    // The lockup program prepends the instruction_data with a tag to tell
    // whitelisted programs that funds are locked, since the instruction data
    // is completely opaque to the lockup program. Without this measure,
    // one would effectively be able to transfer funds from the lockup program
    // freely and use those funds without restriction.
    let (is_locked, instruction_data) = {
        if instruction_data.len() <= 8
            || instruction_data[..8] != serum_lockup::instruction::TAG.to_le_bytes()
        {
            (false, instruction_data)
        } else {
            (true, &instruction_data[8..])
        }
    };

    let instruction: RegistryInstruction = RegistryInstruction::unpack(instruction_data)
        .map_err(|_| RegistryError::ErrorCode(RegistryErrorCode::WrongSerialization))?;

    let result = match instruction {
        RegistryInstruction::Initialize {
            authority,
            nonce,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        } => initialize::handler(
            program_id,
            accounts,
            authority,
            nonce,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        ),
        RegistryInstruction::UpdateRegistrar {
            new_authority,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        } => update_registrar::handler(
            program_id,
            accounts,
            new_authority,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        ),
        RegistryInstruction::CreateEntity { metadata } => {
            create_entity::handler(program_id, accounts, metadata)
        }
        RegistryInstruction::UpdateEntity { leader, metadata } => {
            update_entity::handler(program_id, accounts, leader, metadata)
        }
        RegistryInstruction::CreateMember { delegate } => {
            create_member::handler(program_id, accounts, delegate)
        }
        RegistryInstruction::UpdateMember { delegate, metadata } => {
            update_member::handler(program_id, accounts, delegate, metadata)
        }
        RegistryInstruction::SwitchEntity => switch_entity::handler(program_id, accounts),
        RegistryInstruction::Deposit { amount } => {
            deposit::handler(program_id, accounts, amount, is_locked)
        }
        RegistryInstruction::Withdraw { amount } => {
            withdraw::handler(program_id, accounts, amount, is_locked)
        }
        RegistryInstruction::Stake { amount } => stake::handler(program_id, accounts, amount),
        RegistryInstruction::MarkGeneration => mark_generation::handler(program_id, accounts),
        RegistryInstruction::StartStakeWithdrawal { amount } => {
            start_stake_withdrawal::handler(program_id, accounts, amount)
        }
        RegistryInstruction::EndStakeWithdrawal => {
            end_stake_withdrawal::handler(program_id, accounts)
        }
        RegistryInstruction::Slash { amount } => slash::handler(program_id, accounts, amount),
        RegistryInstruction::DropPoolReward { totals } => {
            drop_pool_reward::handler(program_id, accounts, totals)
        }
        RegistryInstruction::DropLockedReward {
            total,
            end_ts,
            expiry_ts,
            expiry_receiver,
            period_count,
            nonce,
        } => drop_locked_reward::handler(
            program_id,
            accounts,
            total,
            end_ts,
            expiry_ts,
            expiry_receiver,
            period_count,
            nonce,
        ),
        RegistryInstruction::ClaimLockedReward { cursor, nonce } => {
            claim_locked_reward::handler(program_id, accounts, cursor, nonce)
        }
    };

    result?;

    Ok(())
}

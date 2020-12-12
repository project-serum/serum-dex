#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use serum_registry::instruction::RegistryInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::pubkey::Pubkey;

mod claim_locked_reward;
mod claim_unlocked_reward;
mod common;
mod create_entity;
mod create_member;
mod deposit;
mod drop_locked_reward;
mod drop_unlocked_reward;
mod end_stake_withdrawal;
mod expire_locked_reward;
mod expire_unlocked_reward;
mod initialize;
mod stake;
mod start_stake_withdrawal;
mod switch_entity;
mod update_entity;
mod update_member;
mod update_registrar;
mod withdraw;

solana_program::entrypoint!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let instruction: RegistryInstruction = RegistryInstruction::unpack(instruction_data)
        .map_err(|_| RegistryError::ErrorCode(RegistryErrorCode::WrongSerialization))?;

    let result = match instruction {
        RegistryInstruction::Initialize {
            authority,
            mint,
            mint_mega,
            nonce,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
        } => initialize::handler(
            program_id,
            accounts,
            mint,
            mint_mega,
            authority,
            nonce,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
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
        RegistryInstruction::CreateMember => create_member::handler(program_id, accounts),
        RegistryInstruction::UpdateMember { metadata } => {
            update_member::handler(program_id, accounts, metadata)
        }
        RegistryInstruction::Deposit { amount } => deposit::handler(program_id, accounts, amount),
        RegistryInstruction::Withdraw { amount } => withdraw::handler(program_id, accounts, amount),
        RegistryInstruction::Stake { amount, balance_id } => {
            stake::handler(program_id, accounts, amount, balance_id)
        }
        RegistryInstruction::StartStakeWithdrawal { amount, balance_id } => {
            start_stake_withdrawal::handler(program_id, accounts, amount, balance_id)
        }
        RegistryInstruction::EndStakeWithdrawal => {
            end_stake_withdrawal::handler(program_id, accounts)
        }
        RegistryInstruction::SwitchEntity => switch_entity::handler(program_id, accounts),
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
        RegistryInstruction::DropUnlockedReward {
            total,
            expiry_ts,
            expiry_receiver,
            nonce,
        } => drop_unlocked_reward::handler(
            program_id,
            accounts,
            total,
            expiry_ts,
            expiry_receiver,
            nonce,
        ),
        RegistryInstruction::ClaimLockedReward { cursor, nonce } => {
            claim_locked_reward::handler(program_id, accounts, cursor, nonce)
        }
        RegistryInstruction::ClaimUnlockedReward { cursor } => {
            claim_unlocked_reward::handler(program_id, accounts, cursor)
        }
        RegistryInstruction::ExpireUnlockedReward => {
            expire_unlocked_reward::handler(program_id, accounts)
        }
        RegistryInstruction::ExpireLockedReward => {
            expire_locked_reward::handler(program_id, accounts)
        }
    };

    result?;

    Ok(())
}

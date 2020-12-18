use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::{vault, BalanceSandbox, PendingWithdrawal, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    msg!("handler: end_stake_withdrawl");

    let acc_infos = &mut accounts.iter();

    let pending_withdrawal_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let member_vault_acc_info = next_account_info(acc_infos)?;
    let member_vault_pw_acc_info = next_account_info(acc_infos)?;
    let member_vault_authority_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { ref registrar } = access_control(AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        member_vault_acc_info,
        member_vault_pw_acc_info,
        entity_acc_info,
        clock_acc_info,
        program_id,
        member_vault_authority_acc_info,
    })?;

    PendingWithdrawal::unpack_mut(
        &mut pending_withdrawal_acc_info.try_borrow_mut_data()?,
        &mut |pending_withdrawal: &mut PendingWithdrawal| {
            state_transition(StateTransitionRequest {
                pending_withdrawal,
                registrar,
                registrar_acc_info,
                member_vault_acc_info,
                member_vault_pw_acc_info,
                member_vault_authority_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

#[inline(always)]
fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: end_stake_withdrawal");

    let AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        clock_acc_info,
        program_id,
        member_vault_acc_info,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let member = access_control::member_entity(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let pending_withdrawal =
        access_control::pending_withdrawal(pending_withdrawal_acc_info, program_id)?;

    let balance_id = {
        let b = member
            .balances
            .iter()
            .filter(|b| b.owner == pending_withdrawal.balance_id)
            .collect::<Vec<&BalanceSandbox>>();
        let balances = b.first().ok_or(RegistryErrorCode::InvalidBalanceSandbox)?;
        balances.owner
    };
    let (_, is_mega_vault) = access_control::member_vault(
        &member,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
        &balance_id,
    )?;
    let (_, is_mega_vault_pw) = access_control::member_vault_pending_withdrawal(
        &member,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
        &balance_id,
    )?;

    let is_mega = {
        if pending_withdrawal.pool == registrar.pool_mint {
            false
        } else if pending_withdrawal.pool == registrar.pool_mint_mega {
            true
        } else {
            return Err(RegistryErrorCode::InvariantViolation)?;
        }
    };
    if is_mega != is_mega_vault || is_mega != is_mega_vault_pw {
        return Err(RegistryErrorCode::InvalidVault)?;
    }

    let clock = access_control::clock(clock_acc_info)?;
    if clock.unix_timestamp < pending_withdrawal.end_ts {
        return Err(RegistryErrorCode::WithdrawalTimelockNotPassed)?;
    }

    Ok(AccessControlResponse { registrar })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: end_stake_withdrawal");

    let StateTransitionRequest {
        pending_withdrawal,
        registrar,
        registrar_acc_info,
        member_vault_acc_info,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
        token_program_acc_info,
    } = req;

    invoke_token_transfer(
        member_vault_pw_acc_info,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
        pending_withdrawal.amount,
    )?;

    pending_withdrawal.burned = true;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    registrar_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_pw_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct AccessControlResponse {
    registrar: Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_pw_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal: &'c mut PendingWithdrawal,
    registrar: &'c Registrar,
}

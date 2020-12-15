use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::{vault, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> Result<(), RegistryError> {
    msg!("handler: withdraw");

    let acc_infos = &mut accounts.iter();

    // Lockup whitelist relay interface.
    let _vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let member_vault_acc_info = next_account_info(acc_infos)?;
    let member_vault_authority_acc_info = next_account_info(acc_infos)?;

    // Program specfic.
    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { ref registrar } = access_control(AccessControlRequest {
        member_vault_authority_acc_info,
        depositor_acc_info,
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        member_vault_acc_info,
        program_id,
        registrar_acc_info,
        depositor_authority_acc_info,
        amount,
    })?;
    state_transition(StateTransitionRequest {
        amount,
        registrar,
        registrar_acc_info,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        depositor_acc_info,
        token_program_acc_info,
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: withdraw");

    let AccessControlRequest {
        member_vault_authority_acc_info,
        depositor_acc_info,
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        member_vault_acc_info,
        registrar_acc_info,
        depositor_authority_acc_info,
        program_id,
        amount,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }
    if !depositor_authority_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _depositor = access_control::token(depositor_acc_info, depositor_authority_acc_info.key)?;
    let member = access_control::member_join(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let (member_vault, _is_mega) = access_control::member_vault(
        &member,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
        depositor_authority_acc_info.key,
    )?;

    // Withdraw specific.
    //
    // Do we have enough funds for the withdrawal?
    if !member_vault.amount < amount {
        return Err(RegistryErrorCode::InsufficientBalance)?;
    }

    Ok(AccessControlResponse { registrar })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: withdraw");

    let StateTransitionRequest {
        amount,
        registrar,
        registrar_acc_info,
        member_vault_authority_acc_info,
        depositor_acc_info,
        member_vault_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer tokens out.
    invoke_token_transfer(
        member_vault_acc_info,
        depositor_acc_info,
        member_vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
        amount,
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    amount: u64,
}

struct AccessControlResponse {
    registrar: Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    amount: u64,
}

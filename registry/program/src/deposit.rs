use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
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
    msg!("handler: deposit");

    let acc_infos = &mut accounts.iter();

    // Lockup whitelist relay interface.
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

    access_control(AccessControlRequest {
        depositor_authority_acc_info,
        depositor_acc_info,
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        program_id,
        registrar_acc_info,
    })?;
    state_transition(StateTransitionRequest {
        amount,
        depositor_authority_acc_info,
        depositor_acc_info,
        token_program_acc_info,
        member_vault_acc_info,
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    msg!("access-control: deposit");

    let AccessControlRequest {
        depositor_authority_acc_info,
        depositor_acc_info,
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        program_id,
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
    let (_member_vault, _is_mega) = access_control::member_vault(
        &member,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
        depositor_authority_acc_info.key,
    )?;

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: deposit");

    let StateTransitionRequest {
        amount,
        depositor_authority_acc_info,
        depositor_acc_info,
        member_vault_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer tokens in.
    invoke_token_transfer(
        depositor_acc_info,
        member_vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
        &[],
        amount,
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a, 'b> {
    member_vault_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    amount: u64,
}

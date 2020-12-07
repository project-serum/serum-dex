use serum_common::pack::Pack;
use serum_registry::access_control::{self, BalanceSandboxAccInfo};
use serum_registry::accounts::{vault, BalanceSandbox, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use spl_token::instruction as token_instruction;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    msg!("handler: create_member");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let registry_signer_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let mut balances = vec![];
    while acc_infos.len() > 0 {
        balances.push(BalanceSandboxAccInfo {
            owner_acc_info: next_account_info(acc_infos)?,
            spt_acc_info: next_account_info(acc_infos)?,
            spt_mega_acc_info: next_account_info(acc_infos)?,
            vault_acc_info: next_account_info(acc_infos)?,
            vault_mega_acc_info: next_account_info(acc_infos)?,
            vault_stake_acc_info: next_account_info(acc_infos)?,
            vault_stake_mega_acc_info: next_account_info(acc_infos)?,
            vault_pw_acc_info: next_account_info(acc_infos)?,
            vault_pw_mega_acc_info: next_account_info(acc_infos)?,
        })
    }

    let AccessControlResponse { ref registrar } = access_control(AccessControlRequest {
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        registrar_acc_info,
        registry_signer_acc_info,
        rent_acc_info,
        program_id,
        balances: &balances,
    })?;
    Member::unpack_unchecked_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                beneficiary_acc_info,
                member,
                entity_acc_info,
                registrar_acc_info,
                registrar,
                registry_signer_acc_info,
                token_program_acc_info,
                balances: &balances,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

#[inline(never)]
fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: create_member");

    let AccessControlRequest {
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
        registrar_acc_info,
        registry_signer_acc_info,
        program_id,
        balances,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _ = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;

    // CreateMember specific.
    if !rent.is_exempt(member_acc_info.lamports(), member_acc_info.try_data_len()?) {
        return Err(RegistryErrorCode::NotRentExempt)?;
    }
    let mut data: &[u8] = &member_acc_info.try_borrow_data()?;
    let member = Member::unpack_unchecked(&mut data)?;
    if member_acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if member.initialized {
        return Err(RegistryErrorCode::AlreadyInitialized)?;
    }
    // Registry signer.
    let vault_authority = Pubkey::create_program_address(
        &vault::signer_seeds(registrar_acc_info.key, &registrar.nonce),
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultNonce)?;
    if &vault_authority != registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }
    // All balance accounts.
    access_control::balance_sandbox(
        balances,
        registrar_acc_info,
        &registrar,
        registry_signer_acc_info,
        &rent,
        program_id,
    )?;

    Ok(AccessControlResponse { registrar })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: create_member");

    let StateTransitionRequest {
        beneficiary_acc_info,
        member,
        entity_acc_info,
        registrar_acc_info,
        registrar,
        registry_signer_acc_info,
        token_program_acc_info,
        balances,
    } = req;

    member.initialized = true;
    member.registrar = *registrar_acc_info.key;
    member.entity = *entity_acc_info.key;
    member.beneficiary = *beneficiary_acc_info.key;
    member.balances = balances
        .iter()
        .map(|b| {
            approve_delegate(
                beneficiary_acc_info,
                token_program_acc_info,
                registrar_acc_info,
                registrar,
                registry_signer_acc_info,
                b.spt_acc_info,
            )?;
            approve_delegate(
                beneficiary_acc_info,
                token_program_acc_info,
                registrar_acc_info,
                registrar,
                registry_signer_acc_info,
                b.spt_mega_acc_info,
            )?;
            Ok(BalanceSandbox {
                owner: *b.owner_acc_info.key,
                spt: *b.spt_acc_info.key,
                spt_mega: *b.spt_mega_acc_info.key,
                vault: *b.vault_acc_info.key,
                vault_mega: *b.vault_mega_acc_info.key,
                vault_stake: *b.vault_stake_acc_info.key,
                vault_stake_mega: *b.vault_stake_mega_acc_info.key,
                vault_pending_withdrawal: *b.vault_pw_acc_info.key,
                vault_pending_withdrawal_mega: *b.vault_pw_mega_acc_info.key,
            })
        })
        .collect::<Result<Vec<BalanceSandbox>, RegistryError>>()?;

    Ok(())
}

#[inline(always)]
fn approve_delegate<'a, 'b, 'c>(
    beneficiary_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    member_spt_acc_info: &'a AccountInfo<'b>,
) -> Result<(), RegistryError> {
    let approve_instr = token_instruction::approve(
        &spl_token::ID,
        member_spt_acc_info.key,
        &beneficiary_acc_info.key,
        registry_signer_acc_info.key,
        &[],
        0,
    )?;
    solana_sdk::program::invoke_signed(
        &approve_instr,
        &[
            member_spt_acc_info.clone(),
            beneficiary_acc_info.clone(),
            registry_signer_acc_info.clone(),
            token_program_acc_info.clone(),
        ],
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    balances: &'c [BalanceSandboxAccInfo<'a, 'b>],
}

struct AccessControlResponse {
    registrar: Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    beneficiary_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    member: &'c mut Member,
    balances: &'c [BalanceSandboxAccInfo<'a, 'b>],
}

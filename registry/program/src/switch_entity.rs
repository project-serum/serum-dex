use serum_common::pack::*;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, EntityState, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use spl_token::state::Account as TokenAccount;
use std::convert::Into;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    msg!("handler: switch_entity");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let curr_entity_acc_info = next_account_info(acc_infos)?;
    let new_entity_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let mut asset_acc_infos = vec![];
    while acc_infos.len() > 0 {
        asset_acc_infos.push(AssetAccInfos {
            owner_acc_info: next_account_info(acc_infos)?,
            spt_acc_info: next_account_info(acc_infos)?,
            spt_mega_acc_info: next_account_info(acc_infos)?,
        });
    }

    let AccessControlResponse {
        ref assets,
        ref registrar,
        ref clock,
    } = access_control(AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        program_id,
        registrar_acc_info,
        curr_entity_acc_info,
        new_entity_acc_info,
        clock_acc_info,
        asset_acc_infos,
        vault_authority_acc_info,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            Entity::unpack_unchecked_mut(
                &mut curr_entity_acc_info.try_borrow_mut_data()?,
                &mut |curr_entity: &mut Entity| {
                    Entity::unpack_unchecked_mut(
                        &mut new_entity_acc_info.try_borrow_mut_data()?,
                        &mut |new_entity: &mut Entity| {
                            state_transition(StateTransitionRequest {
                                new_entity_acc_info,
                                curr_entity,
                                new_entity,
                                member,
                                registrar,
                                clock,
                                assets,
                            })
                            .map_err(Into::into)
                        },
                    )
                },
            )
        },
    )?;

    Ok(())
}

#[inline(never)]
fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: switch_entity");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        program_id,
        registrar_acc_info,
        curr_entity_acc_info,
        new_entity_acc_info,
        clock_acc_info,
        asset_acc_infos,
        vault_authority_acc_info,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let clock = access_control::clock(clock_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let member = access_control::member_join(
        member_acc_info,
        curr_entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let _curr_entity =
        access_control::entity(curr_entity_acc_info, registrar_acc_info, program_id)?;
    let _new_entity = access_control::entity(new_entity_acc_info, registrar_acc_info, program_id)?;
    let mut balance_ids: Vec<Pubkey> = asset_acc_infos
        .iter()
        .map(|a| *a.owner_acc_info.key)
        .collect();
    balance_ids.sort();
    balance_ids.dedup();
    if balance_ids.len() != member.balances.len() {
        return Err(RegistryErrorCode::InvalidAssetsLen)?;
    }
    // BPF exploads when mapping so use a for loop.
    let mut assets = vec![];
    for a in &asset_acc_infos {
        let (spt, is_mega) = access_control::member_spt(
            &member,
            a.spt_acc_info,
            vault_authority_acc_info,
            registrar_acc_info,
            &registrar,
            program_id,
            a.owner_acc_info.key,
        )?;
        assert!(!is_mega);
        let (spt_mega, is_mega) = access_control::member_spt(
            &member,
            a.spt_mega_acc_info,
            vault_authority_acc_info,
            registrar_acc_info,
            &registrar,
            program_id,
            a.owner_acc_info.key,
        )?;
        assert!(is_mega);
        assets.push(Assets { spt, spt_mega });
    }

    Ok(AccessControlResponse {
        assets,
        registrar,
        clock,
    })
}

#[inline(never)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: switch_entity");

    let StateTransitionRequest {
        new_entity_acc_info,
        mut member,
        curr_entity,
        new_entity,
        registrar,
        clock,
        assets,
    } = req;

    // Bump the last stake timestamp to prevent people from switching from
    // inactive to active entities to retrieve a reward when they shouldn't.
    if curr_entity.state == EntityState::Inactive {
        member.last_stake_ts = clock.unix_timestamp;
    }

    // Bookepping.
    //
    // Move all the assets to the new entity.
    //
    // Note that the assets don't actually move, as the member vaults are
    // untouched.
    for a in assets {
        // Remove.
        curr_entity.balances.spt_amount -= a.spt.amount;
        curr_entity.balances.spt_mega_amount -= a.spt_mega.amount;
        // Add.
        new_entity.balances.spt_amount += a.spt.amount;
        new_entity.balances.spt_mega_amount += a.spt_mega.amount;
    }

    member.entity = *new_entity_acc_info.key;

    // Trigger activation FSM.
    curr_entity.transition_activation_if_needed(registrar, clock);
    new_entity.transition_activation_if_needed(registrar, clock);

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    curr_entity_acc_info: &'a AccountInfo<'b>,
    new_entity_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    asset_acc_infos: Vec<AssetAccInfos<'a, 'b>>,
}

struct AccessControlResponse {
    registrar: Registrar,
    clock: Clock,
    assets: Vec<Assets>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    new_entity_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    curr_entity: &'c mut Entity,
    new_entity: &'c mut Entity,
    member: &'c mut Member,
    clock: &'c Clock,
    assets: &'c [Assets],
}

struct Assets {
    spt: TokenAccount,
    spt_mega: TokenAccount,
}

struct AssetAccInfos<'a, 'b> {
    owner_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    spt_mega_acc_info: &'a AccountInfo<'b>,
}

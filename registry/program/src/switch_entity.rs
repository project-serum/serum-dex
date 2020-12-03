use serum_common::pack::*;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use std::convert::Into;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    info!("handler: switch_entity");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let curr_entity_acc_info = next_account_info(acc_infos)?;
    let new_entity_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { registrar, clock } = access_control(AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        program_id,
        registrar_acc_info,
        curr_entity_acc_info,
        new_entity_acc_info,
        clock_acc_info,
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
                                registrar: &registrar,
                                clock: &clock,
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

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: switch_entity");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        program_id,
        registrar_acc_info,
        curr_entity_acc_info,
        new_entity_acc_info,
        clock_acc_info,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let clock = access_control::clock(clock_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _member = access_control::member_join(
        member_acc_info,
        curr_entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let _curr_entity =
        access_control::entity(curr_entity_acc_info, registrar_acc_info, program_id)?;
    let _new_entity = access_control::entity(new_entity_acc_info, registrar_acc_info, program_id)?;

    Ok(AccessControlResponse { registrar, clock })
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: switch_entity");

    let StateTransitionRequest {
        new_entity_acc_info,
        mut member,
        curr_entity,
        new_entity,
        registrar,
        clock,
    } = req;

    curr_entity.remove(member);
    new_entity.add(member);

    curr_entity.transition_activation_if_needed(registrar, clock);
    new_entity.transition_activation_if_needed(registrar, clock);

    member.entity = *new_entity_acc_info.key;
    member.last_stake_ts = clock.unix_timestamp;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    curr_entity_acc_info: &'a AccountInfo<'b>,
    new_entity_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct AccessControlResponse {
    registrar: Registrar,
    clock: Clock,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    new_entity_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    curr_entity: &'c mut Entity,
    new_entity: &'c mut Entity,
    member: &'c mut Member,
    clock: &'c Clock,
}

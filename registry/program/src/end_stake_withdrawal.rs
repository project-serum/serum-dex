use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Member, PendingWithdrawal};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    info!("handler: end_stake_withdrawl");

    let acc_infos = &mut accounts.iter();

    let pending_withdrawal_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        clock_acc_info,
        program_id,
    })?;

    Entity::unpack_unchecked_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            Member::unpack_mut(
                &mut member_acc_info.try_borrow_mut_data()?,
                &mut |member: &mut Member| {
                    PendingWithdrawal::unpack_mut(
                        &mut pending_withdrawal_acc_info.try_borrow_mut_data()?,
                        &mut |pending_withdrawal: &mut PendingWithdrawal| {
                            state_transition(StateTransitionRequest {
                                pending_withdrawal,
                                entity,
                                member,
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

#[inline(always)]
fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: end_stake_withdrawal");

    let AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        clock_acc_info,
        program_id,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let _registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let _member = access_control::member_join(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let pending_withdrawal =
        access_control::pending_withdrawal(pending_withdrawal_acc_info, program_id)?;
    let clock = access_control::clock(clock_acc_info)?;

    // EndStakeWithdrawal specific.
    {
        if clock.unix_timestamp < pending_withdrawal.end_ts {
            return Err(RegistryErrorCode::WithdrawalTimelockNotPassed)?;
        }
    }

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: end_stake_withdrawal");

    let StateTransitionRequest {
        pending_withdrawal,
        entity,
        member,
    } = req;

    member.spt_did_redeem_end(
        pending_withdrawal.payment.asset_amount,
        pending_withdrawal.payment.mega_asset_amount,
    );
    entity.spt_did_redeem_end(
        pending_withdrawal.payment.asset_amount,
        pending_withdrawal.payment.mega_asset_amount,
    );
    pending_withdrawal.burned = true;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    registrar_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a> {
    pending_withdrawal: &'a mut PendingWithdrawal,
    entity: &'a mut Entity,
    member: &'a mut Member,
}

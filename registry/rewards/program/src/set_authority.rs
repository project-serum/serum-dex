use crate::access_control;
use serum_common::pack::*;
use serum_registry_rewards::accounts::Instance;
use serum_registry_rewards::error::RewardsError;
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_authority: Pubkey,
) -> Result<(), RewardsError> {
    msg!("handler: set_authority");

    let acc_infos = &mut accounts.iter();

    let instance_authority_acc_info = next_account_info(acc_infos)?;
    let instance_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    })?;

    Instance::unpack_mut(
        &mut instance_acc_info.try_borrow_mut_data()?,
        &mut |instance: &mut Instance| {
            state_transition(StateTransitionRequest {
                instance,
                new_authority,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    msg!("access-control: set_authority");

    let AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    } = req;

    let instance = access_control::instance(instance_acc_info, program_id)?;
    access_control::governance(program_id, instance_authority_acc_info, &instance)?;

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    msg!("state-transition: set_authority");

    let StateTransitionRequest {
        instance,
        new_authority,
    } = req;

    instance.authority = new_authority;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    instance_acc_info: &'a AccountInfo<'b>,
    instance_authority_acc_info: &'a AccountInfo<'b>,
}

struct StateTransitionRequest<'a> {
    instance: &'a mut Instance,
    new_authority: Pubkey,
}

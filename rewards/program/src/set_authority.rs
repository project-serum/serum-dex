use crate::access_control;
use serum_common::pack::*;
use serum_rewards::accounts::Instance;
use serum_rewards::error::RewardsError;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    new_authority: Pubkey,
) -> Result<(), RewardsError> {
    info!("handler: set_authority");

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
    info!("access-control: set_authority");

    let AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    } = req;

    let _ = access_control::governance(program_id, instance_acc_info, instance_authority_acc_info)?;

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: set_authority");

    let StateTransitionRequest {
        instance,
        new_authority,
    } = req;

    instance.authority = new_authority;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    instance_acc_info: &'a AccountInfo<'a>,
    instance_authority_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    instance: &'a mut Instance,
    new_authority: Pubkey,
}

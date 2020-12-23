use crate::access_control;
use serum_common::pack::*;
use serum_registry_rewards::accounts::Instance;
use serum_registry_rewards::error::RewardsError;
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    fee_rate: u64,
) -> Result<(), RewardsError> {
    msg!("handler: set_fee_rate");

    let acc_infos = &mut accounts.iter();

    let instance_authority_acc_info = next_account_info(acc_infos)?;
    let instance_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    })?;

    state_transition(StateTransitionRequest {
        instance_acc_info,
        fee_rate,
    })
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    msg!("access-control: set_fee_rate");

    let AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    } = req;

    let instance = access_control::instance(instance_acc_info, program_id)?;
    // Authorization.
    access_control::governance(instance_authority_acc_info, &instance)?;

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    msg!("state-transition: set_fee_rate");

    let StateTransitionRequest {
        instance_acc_info,
        fee_rate,
    } = req;

    Instance::unpack_mut(
        &mut instance_acc_info.try_borrow_mut_data()?,
        &mut |instance: &mut Instance| {
            instance.fee_rate = fee_rate;
            Ok(())
        },
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    instance_acc_info: &'a AccountInfo<'b>,
    instance_authority_acc_info: &'a AccountInfo<'b>,
}

struct StateTransitionRequest<'a, 'b> {
    instance_acc_info: &'a AccountInfo<'b>,
    fee_rate: u64,
}

use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::Safe;
use serum_lockup::error::LockupError;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    new_authority: Pubkey,
) -> Result<(), LockupError> {
    info!("handler: set_authority");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_authority_acc_info,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe_acc: &mut Safe| {
            state_transition(StateTransitionRequest {
                safe_acc,
                new_authority,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), LockupError> {
    info!("access-control: set_authority");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_authority_acc_info,
    } = req;

    // Governance authorization.
    let _ = access_control::governance(program_id, safe_acc_info, safe_authority_acc_info)?;

    info!("access-control: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_authority_acc_info: &'a AccountInfo<'a>,
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), LockupError> {
    info!("state-transition: set_authority");

    let StateTransitionRequest {
        safe_acc,
        new_authority,
    } = req;

    safe_acc.authority = new_authority;

    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a> {
    safe_acc: &'a mut Safe,
    new_authority: Pubkey,
}

use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::Whitelist;
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    program_id_to_delete: Pubkey,
) -> Result<(), LockupError> {
    info!("handler: whitelist_delete");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let whitelist_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
        program_id_to_delete,
    })?;

    Whitelist::unpack_mut(
        &mut whitelist_acc_info.try_borrow_mut_data()?,
        &mut |whitelist: &mut Whitelist| {
            state_transition(StateTransitionRequest {
                whitelist,
                program_id_to_delete,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: whitelist_delete");

    let AccessControlRequest {
        program_id,
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
        program_id_to_delete,
    } = req;

    // Governance authorization.
    let safe = access_control::governance(program_id, safe_acc_info, safe_authority_acc_info)?;

    // WhitelistDelete checks.
    let whitelist = access_control::whitelist(whitelist_acc_info, &safe, program_id)?;
    if !whitelist.contains(&program_id_to_delete) {
        return Err(LockupErrorCode::WhitelistProgramNotFound)?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: whitelist_delete");

    let StateTransitionRequest {
        whitelist,
        program_id_to_delete,
    } = req;

    whitelist
        .delete(program_id_to_delete)
        .ok_or(LockupErrorCode::WhitelistProgramNotFound)?;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    program_id_to_delete: Pubkey,
    safe_authority_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    whitelist_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    whitelist: &'a mut Whitelist,
    program_id_to_delete: Pubkey,
}

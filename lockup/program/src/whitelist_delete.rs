use crate::access_control;
use serum_lockup::accounts::{Whitelist, WhitelistEntry};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    wl_entry: WhitelistEntry,
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
    })?;

    let whitelist = Whitelist::new(whitelist_acc_info.clone())?;

    state_transition(StateTransitionRequest {
        whitelist,
        wl_entry,
    })
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: whitelist_delete");

    let AccessControlRequest {
        program_id,
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
    } = req;

    // Governance authorization.
    let safe = access_control::governance(program_id, safe_acc_info, safe_authority_acc_info)?;

    // WhitelistDelete checks.
    let _ =
        access_control::whitelist(whitelist_acc_info.clone(), safe_acc_info, &safe, program_id)?;

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: whitelist_delete");

    let StateTransitionRequest {
        whitelist,
        wl_entry,
    } = req;

    whitelist
        .delete(wl_entry)?
        .ok_or(LockupErrorCode::WhitelistNotFound)?;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_authority_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    whitelist_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    whitelist: Whitelist<'a>,
    wl_entry: WhitelistEntry,
}

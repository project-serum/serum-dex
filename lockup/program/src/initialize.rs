use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, Whitelist};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: Pubkey,
) -> Result<(), LockupError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let safe_acc_info = next_account_info(acc_infos)?;
    let whitelist_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        whitelist_acc_info,
        rent_acc_info,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe: &mut Safe| {
            state_transition(StateTransitionRequest {
                safe,
                safe_addr: safe_acc_info.key,
                whitelist: Whitelist::new(whitelist_acc_info.clone())?,
                whitelist_addr: whitelist_acc_info.key,
                authority,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        rent_acc_info,
        whitelist_acc_info,
    } = req;

    // Authorization: none.

    // Rent.
    let rent = access_control::rent(rent_acc_info)?;

    // Safe (uninitialized).
    {
        let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
        if safe_acc_info.owner != program_id {
            return Err(LockupErrorCode::NotOwnedByProgram)?;
        }
        if !rent.is_exempt(safe_acc_info.lamports(), safe_acc_info.try_data_len()?) {
            return Err(LockupErrorCode::NotRentExempt)?;
        }
        if safe.initialized {
            return Err(LockupErrorCode::AlreadyInitialized)?;
        }
    }

    // Whitelist (not yet set on Safe).
    {
        if whitelist_acc_info.owner != program_id {
            return Err(LockupErrorCode::InvalidAccountOwner)?;
        }
        if !rent.is_exempt(
            whitelist_acc_info.lamports(),
            whitelist_acc_info.try_data_len()?,
        ) {
            return Err(LockupErrorCode::NotRentExempt)?;
        }
        if Pubkey::new_from_array([0; 32]) != Whitelist::new(whitelist_acc_info.clone())?.safe()? {
            return Err(LockupErrorCode::WhitelistAlreadyInitialized)?;
        }
    }

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        safe,
        safe_addr,
        authority,
        whitelist,
        whitelist_addr,
    } = req;

    // Initialize Safe.
    safe.initialized = true;
    safe.authority = authority;
    safe.whitelist = *whitelist_addr;

    // Inittialize Whitelist.
    whitelist.set_safe(safe_addr)?;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'b>,
    whitelist_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
}

struct StateTransitionRequest<'a, 'b> {
    safe: &'a mut Safe,
    safe_addr: &'a Pubkey,
    whitelist_addr: &'a Pubkey,
    whitelist: Whitelist<'b>,
    authority: Pubkey,
}

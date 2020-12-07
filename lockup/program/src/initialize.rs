use crate::common::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, Whitelist};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: Pubkey,
) -> Result<(), LockupError> {
    msg!("handler: initialize");

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
                authority,
                safe,
                safe_acc_info,
                whitelist_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    msg!("access-control: initialize");

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

    // Whitelist (uninitialized).
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
    msg!("state-transition: initialize");

    let StateTransitionRequest {
        safe,
        safe_acc_info,
        authority,
        whitelist_acc_info,
    } = req;

    // Initialize Safe.
    safe.initialized = true;
    safe.authority = authority;
    safe.whitelist = *whitelist_acc_info.key;

    // Inittialize Whitelist.
    let whitelist = Whitelist::new(whitelist_acc_info.clone())?;
    whitelist.set_safe(safe_acc_info.key)?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    safe_acc_info: &'a AccountInfo<'b>,
    whitelist_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a, 'b> {
    safe_acc_info: &'a AccountInfo<'b>,
    whitelist_acc_info: &'a AccountInfo<'b>,
    safe: &'a mut Safe,
    authority: Pubkey,
}

//! api.rs defines all instruction handlers for the program.

use serum_safe::accounts::{SafeAccount, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::{IsInitialized, Pack};

// questions:
// * Can i format strings here? Solana seems to break.
//
// todo: add program_id and maybe accounts to global data that can be queried
//       via sol_env!().program_id, instead of passing them into the functions.
pub fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: Pubkey,
) -> Result<(), SafeError> {
    info!("initializing the safe");
    let account_info_iter = &mut accounts.iter();
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_account_data_len = safe_account_info.data_len();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    let mut safe_account_data = safe_account_info.data.borrow_mut();
    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            if safe_account.is_initialized {
                info!("ERROR: safe account already initialized");
                return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized).into());
            }
            if !rent.is_exempt(safe_account_info.lamports(), safe_account_data_len) {
                info!("ERROR: safe account is not rent exempt");
                return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
            }
            if safe_account_info.owner != program_id {
                info!("ERROR: safe account owner is not the program id");
                return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram).into());
            }

            safe_account.is_initialized = true;
            safe_account.supply = 0;
            safe_account.authority = authority;

            info!("safe initialization complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

pub fn slash(accounts: &[AccountInfo]) -> Result<(), SafeError> {
    // todo
    Ok(())
}

pub fn deposit_srm(
    accounts: &[AccountInfo],
    user_spl_wallet_owner: Pubkey,
    slot_number: u64,
    amount: u64,
    lsrm_amount: u64,
) -> Result<(), SafeError> {
    info!("**********deposit SRM!");
    Ok(())
}

pub fn withdraw_srm(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    info!("**********withdraw SRM!");
    Ok(())
}

pub fn mint_locked_srm(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    info!("**********mint SRM!");
    Ok(())
}

pub fn burn_locked_srm(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    info!("**********burn SRM!");
    Ok(())
}

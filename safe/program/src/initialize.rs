use serum_safe::accounts::{SafeAccount, SrmVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    mint: Pubkey,
    authority: Pubkey,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("HANDLER: initialize");
    let account_info_iter = &mut accounts.iter();
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_account_data_len = safe_account_info.data_len();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    let mut safe_account_data = safe_account_info.try_borrow_mut_data()?;
    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            initialize_access_control(
                program_id,
                safe_account,
                safe_account_info,
                safe_account_data_len,
                rent,
                nonce,
            )?;

            safe_account.mint = mint;
            safe_account.is_initialized = true;
            safe_account.supply = 0;
            safe_account.authority = authority;
            safe_account.nonce = nonce;
            // todo: consider adding the vault to the safe account directly
            //       if we do that, then check the owner in access control

            info!("safe initialization complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn initialize_access_control(
    program_id: &Pubkey,
    safe_account: &SafeAccount,
    safe_account_info: &AccountInfo,
    safe_account_data_len: usize,
    rent: &Rent,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("ACCESS CONTROL: initialize");
    if safe_account.is_initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized));
    }
    if !rent.is_exempt(safe_account_info.lamports(), safe_account_data_len) {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt));
    }
    if safe_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram));
    }
    if Pubkey::create_program_address(
        &SrmVault::signer_seeds(safe_account_info.key, &[nonce]),
        program_id,
    )
    .is_err()
    {
        return Err(SafeError::ErrorCode(SafeErrorCode::InvalidVaultNonce));
    }
    info!("ACCESS CONTROL: success");
    Ok(())
}

use serum_safe::accounts::{SafeAccount, SrmVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    mint: Pubkey,
    authority: Pubkey,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let safe_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        rent_acc_info,
        nonce,
    })?;

    SafeAccount::unpack_unchecked_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe: &mut SafeAccount| {
            state_transition(StateTransitionRequest {
                safe,
                mint,
                authority,
                nonce,
            })?;
            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        rent_acc_info,
        nonce,
    } = req;

    let safe_data = safe_acc_info.try_borrow_data()?;
    let safe = SafeAccount::unpack_unchecked(&safe_data)?;
    if safe.is_initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized));
    }
    let rent = Rent::from_account_info(rent_acc_info)?;
    if !rent.is_exempt(safe_acc_info.lamports(), safe_data.len()) {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt));
    }
    if safe_acc_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram));
    }
    if Pubkey::create_program_address(
        &SrmVault::signer_seeds(safe_acc_info.key, &[nonce]),
        program_id,
    )
    .is_err()
    {
        return Err(SafeError::ErrorCode(SafeErrorCode::InvalidVaultNonce));
    }
    info!("access-control: success");
    Ok(())
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), SafeError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        safe,
        mint,
        authority,
        nonce,
    } = req;

    safe.mint = mint;
    safe.is_initialized = true;
    safe.supply = 0;
    safe.authority = authority;
    safe.nonce = nonce;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    nonce: u8,
}

struct StateTransitionRequest<'a> {
    safe: &'a mut SafeAccount,
    mint: Pubkey,
    authority: Pubkey,
    nonce: u8,
}

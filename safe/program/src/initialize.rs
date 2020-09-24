use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack as TokenPack;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    authority: Pubkey,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let safe_acc_info = next_account_info(acc_infos)?;
    let mint_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        mint_acc_info,
        rent_acc_info,
        nonce,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe: &mut Safe| {
            state_transition(StateTransitionRequest {
                safe,
                mint: mint_acc_info.key,
                authority,
                nonce,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        mint_acc_info,
        rent_acc_info,
        nonce,
    } = req;

    let safe_data = safe_acc_info.try_borrow_data()?;
    let safe = Safe::unpack(&safe_data)?;
    if safe.initialized {
        return Err(SafeErrorCode::AlreadyInitialized)?;
    }
    if safe_acc_info.owner != program_id {
        return Err(SafeErrorCode::NotOwnedByProgram)?;
    }
    let rent = Rent::from_account_info(rent_acc_info)?;
    if !rent.is_exempt(safe_acc_info.lamports(), safe_data.len()) {
        return Err(SafeErrorCode::NotRentExempt)?;
    }
    if Pubkey::create_program_address(
        &TokenVault::signer_seeds(safe_acc_info.key, &nonce),
        program_id,
    )
    .is_err()
    {
        return Err(SafeErrorCode::InvalidVaultNonce)?;
    }
    let mint = spl_token::state::Mint::unpack(&mint_acc_info.try_borrow_data()?)?;
    if !mint.is_initialized {
        return Err(SafeErrorCode::UnitializedTokenMint)?;
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

    safe.mint = *mint;
    safe.initialized = true;
    safe.authority = authority;
    safe.nonce = nonce;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    nonce: u8,
}

struct StateTransitionRequest<'a> {
    safe: &'a mut Safe,
    mint: &'a Pubkey,
    authority: Pubkey,
    nonce: u8,
}

use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, TokenVault};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    authority: Pubkey,
    nonce: u8,
) -> Result<(), LockupError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let safe_acc_info = next_account_info(acc_infos)?;
    let whitelist_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let mint_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        vault_acc_info,
        safe_acc_info,
        whitelist_acc_info,
        mint_acc_info,
        rent_acc_info,
        nonce,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe: &mut Safe| {
            state_transition(StateTransitionRequest {
                safe,
                whitelist: whitelist_acc_info.key,
                mint: mint_acc_info.key,
                vault: *vault_acc_info.key,
                authority,
                nonce,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), LockupError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        mint_acc_info,
        vault_acc_info,
        rent_acc_info,
        whitelist_acc_info,
        nonce,
    } = req;

    // Authorization: none.

    // Rent.
    let rent = access_control::rent(rent_acc_info)?;

    // Safe.
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

    // Whitelist.
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
    }

    // Vault.
    {
        let vault = access_control::token(vault_acc_info)?;
        let vault_authority = Pubkey::create_program_address(
            &TokenVault::signer_seeds(safe_acc_info.key, &nonce),
            program_id,
        )
        .map_err(|_| LockupErrorCode::InvalidVaultNonce)?;

        if vault.owner != vault_authority {
            return Err(LockupErrorCode::InvalidVault)?;
        }
        if !rent.is_exempt(vault_acc_info.lamports(), vault_acc_info.try_data_len()?) {
            return Err(LockupErrorCode::NotRentExempt)?;
        }
    }

    // Mint.
    let _ = access_control::mint(mint_acc_info)?;

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), LockupError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        safe,
        mint,
        authority,
        nonce,
        whitelist,
        vault,
    } = req;

    safe.initialized = true;
    safe.mint = *mint;
    safe.authority = authority;
    safe.nonce = nonce;
    safe.whitelist = *whitelist;
    safe.vault = vault;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    whitelist_acc_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    nonce: u8,
}

struct StateTransitionRequest<'a> {
    safe: &'a mut Safe,
    whitelist: &'a Pubkey,
    mint: &'a Pubkey,
    authority: Pubkey,
    vault: Pubkey,
    nonce: u8,
}

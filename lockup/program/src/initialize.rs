use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, TokenVault, Whitelist};
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
                safe_addr: safe_acc_info.key,
                whitelist: Whitelist::new(whitelist_acc_info.clone())?,
                whitelist_addr: whitelist_acc_info.key,
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

    // Vault (initialized but not yet on Safe).
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

    // Mint (initialized but not yet on Safe).
    let _ = access_control::mint(mint_acc_info)?;

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        safe,
        safe_addr,
        mint,
        authority,
        nonce,
        whitelist,
        whitelist_addr,
        vault,
    } = req;

    // Initialize Safe.
    safe.initialized = true;
    safe.mint = *mint;
    safe.authority = authority;
    safe.nonce = nonce;
    safe.whitelist = *whitelist_addr;
    safe.vault = vault;

    // Inittialize Whitelist.
    whitelist.set_safe(safe_addr)?;

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

struct StateTransitionRequest<'a, 'b> {
    safe: &'a mut Safe,
    safe_addr: &'a Pubkey,
    whitelist_addr: &'a Pubkey,
    whitelist: Whitelist<'b>,
    mint: &'a Pubkey,
    authority: Pubkey,
    vault: Pubkey,
    nonce: u8,
}

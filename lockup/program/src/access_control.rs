//! Module for safe access to accounts.

use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, TokenVault, Vesting, Whitelist};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_option::COption;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::state::{Account as TokenAccount, Mint};
use std::convert::Into;

pub fn governance(
    program_id: &Pubkey,
    safe_acc_info: &AccountInfo,
    safe_authority_acc_info: &AccountInfo,
) -> Result<Safe, LockupError> {
    if !safe_authority_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }
    let safe = safe(safe_acc_info, program_id)?;
    if safe.authority != *safe_authority_acc_info.key {
        return Err(LockupErrorCode::Unauthorized)?;
    }
    Ok(safe)
}

pub fn whitelist<'a>(
    wl_acc_info: AccountInfo<'a>,
    safe_acc_info: &AccountInfo<'a>,
    safe: &Safe,
    program_id: &Pubkey,
) -> Result<Whitelist<'a>, LockupError> {
    if program_id != wl_acc_info.owner {
        return Err(LockupErrorCode::InvalidAccountOwner)?;
    }

    if safe.whitelist != *wl_acc_info.key {
        return Err(LockupErrorCode::InvalidWhitelist)?;
    }
    let wl = Whitelist::new(wl_acc_info)?;
    if wl.safe()? != *safe_acc_info.key {
        return Err(LockupErrorCode::WhitelistSafeMismatch)?;
    }

    Ok(wl)
}

/// Access control on any instruction mutating an existing Vesting account.
pub fn vesting(
    program_id: &Pubkey,
    safe: &Pubkey,
    vesting_acc_info: &AccountInfo,
    vesting_acc_beneficiary_info: &AccountInfo,
) -> Result<Vesting, LockupError> {
    let vesting = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;

    if vesting_acc_info.owner != program_id {
        return Err(LockupErrorCode::InvalidAccount)?;
    }
    if !vesting.initialized {
        return Err(LockupErrorCode::NotInitialized)?;
    }
    if vesting.beneficiary != *vesting_acc_beneficiary_info.key {
        return Err(LockupErrorCode::Unauthorized)?;
    }
    if vesting.safe != *safe {
        return Err(LockupErrorCode::WrongSafe)?;
    }

    Ok(vesting)
}

pub fn rent(acc_info: &AccountInfo) -> Result<Rent, LockupError> {
    if *acc_info.key != solana_sdk::sysvar::rent::id() {
        return Err(LockupErrorCode::InvalidRentSysvar)?;
    }
    Rent::from_account_info(acc_info).map_err(Into::into)
}

pub fn clock(acc_info: &AccountInfo) -> Result<Clock, LockupError> {
    if *acc_info.key != solana_sdk::sysvar::clock::id() {
        return Err(LockupErrorCode::InvalidClockSysvar)?;
    }
    Clock::from_account_info(acc_info).map_err(Into::into)
}

pub fn mint(acc_info: &AccountInfo) -> Result<Mint, LockupError> {
    if *acc_info.owner != spl_token::ID {
        return Err(LockupErrorCode::InvalidMint)?;
    }

    let mint = Mint::unpack(&acc_info.try_borrow_data()?)?;
    if !mint.is_initialized {
        return Err(LockupErrorCode::UnitializedTokenMint)?;
    }

    Ok(mint)
}

pub fn safe(acc_info: &AccountInfo, program_id: &Pubkey) -> Result<Safe, LockupError> {
    if acc_info.owner != program_id {
        return Err(LockupErrorCode::InvalidAccountOwner)?;
    }

    let safe = Safe::unpack(&acc_info.try_borrow_data()?)?;
    if !safe.initialized {
        return Err(LockupErrorCode::NotInitialized)?;
    }

    Ok(safe)
}

pub fn vault(
    acc_info: &AccountInfo,
    vault_authority_acc_info: &AccountInfo,
    safe_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<TokenAccount, LockupError> {
    let safe = safe(safe_acc_info, program_id)?;
    let vault = token(acc_info)?;
    if *acc_info.key != safe.vault {
        return Err(LockupErrorCode::InvalidVault)?;
    }

    let va = vault_authority(
        vault_authority_acc_info,
        safe_acc_info.key,
        &safe,
        program_id,
    )?;

    if va != vault.owner {
        return Err(LockupErrorCode::InvalidVault)?;
    }
    if va != *vault_authority_acc_info.key {
        return Err(LockupErrorCode::InvalidVault)?;
    }

    Ok(vault)
}

pub fn vault_authority(
    vault_authority_acc_info: &AccountInfo,
    safe_addr: &Pubkey,
    safe: &Safe,
    program_id: &Pubkey,
) -> Result<Pubkey, LockupError> {
    let va = Pubkey::create_program_address(
        &TokenVault::signer_seeds(safe_addr, &safe.nonce),
        program_id,
    )
    .map_err(|_| LockupErrorCode::InvalidVaultNonce)?;
    if va != *vault_authority_acc_info.key {
        return Err(LockupErrorCode::InvalidVault)?;
    }

    Ok(va)
}

pub fn token(acc_info: &AccountInfo) -> Result<TokenAccount, LockupError> {
    if *acc_info.owner != spl_token::ID {
        return Err(LockupErrorCode::InvalidAccountOwner)?;
    }

    let token = TokenAccount::unpack(&acc_info.try_borrow_data()?)?;
    if token.state != spl_token::state::AccountState::Initialized {
        return Err(LockupErrorCode::NotInitialized)?;
    }

    Ok(token)
}

pub fn locked_token(
    acc_info: &AccountInfo,
    mint_acc_info: &AccountInfo,
    vault_authority: &Pubkey,
    vesting: &Vesting,
) -> Result<TokenAccount, LockupError> {
    // Mint.
    let mint = mint(mint_acc_info)?;
    if mint.mint_authority != COption::Some(*vault_authority) {
        return Err(LockupErrorCode::InvalidMintAuthority)?;
    }

    // Token.
    let token_acc = token(acc_info)?;
    if token_acc.owner != vesting.beneficiary {
        return Err(LockupErrorCode::InvalidTokenAccountOwner)?;
    }
    if token_acc.mint != vesting.locked_nft_mint {
        return Err(LockupErrorCode::InvalidTokenAccountMint)?;
    }
    if token_acc.mint != *mint_acc_info.key {
        return Err(LockupErrorCode::InvalidMint)?;
    }

    Ok(token_acc)
}

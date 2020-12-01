//! Module for safe access to accounts.

use serum_common::pack::Pack;
use serum_lockup::accounts::{vault, Safe, Vesting, Whitelist};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::state::Account as TokenAccount;
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
    let vesting = vesting_raw(program_id, safe, vesting_acc_info)?;

    if vesting.beneficiary != *vesting_acc_beneficiary_info.key {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    Ok(vesting)
}

pub fn vesting_raw(
    program_id: &Pubkey,
    safe: &Pubkey,
    vesting_acc_info: &AccountInfo,
) -> Result<Vesting, LockupError> {
    let mut data: &[u8] = &vesting_acc_info.try_borrow_data()?;
    let vesting = Vesting::unpack_unchecked(&mut data)?;

    if vesting_acc_info.owner != program_id {
        return Err(LockupErrorCode::InvalidAccount)?;
    }
    if !vesting.initialized {
        return Err(LockupErrorCode::NotInitialized)?;
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
    vesting_acc_info: &AccountInfo,
    beneficiary_acc_info: &AccountInfo,
    safe_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<TokenAccount, LockupError> {
    let vesting = vesting_raw(program_id, safe_acc_info.key, vesting_acc_info)?;
    let vault = token(acc_info)?;
    let va = vault_authority(
        program_id,
        vault_authority_acc_info,
        beneficiary_acc_info,
        &vesting,
        safe_acc_info.key,
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
    program_id: &Pubkey,
    vault_authority_acc_info: &AccountInfo,
    beneficiary_acc_info: &AccountInfo,
    vesting: &Vesting,
    safe_addr: &Pubkey,
) -> Result<Pubkey, LockupError> {
    let va = Pubkey::create_program_address(
        &vault::signer_seeds(safe_addr, beneficiary_acc_info.key, &vesting.nonce),
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

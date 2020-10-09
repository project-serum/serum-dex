//! Module for shared access control accross instruction handlers.

use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::state::{Account, Mint};
use std::convert::Into;

/// Access control on any instruction mutating the Whitelist account, requiring
/// the authority's signature.
pub fn whitelist_gov(req: WhitelistGovRequest) -> Result<(), LockupError> {
    let WhitelistGovRequest {
        program_id,
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
    } = req;

    // Safe authority authorization.
    {
        if !safe_authority_acc_info.is_signer {
            return Err(LockupErrorCode::Unauthorized)?;
        }
    }

    // Safe.
    let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
    {
        if safe.authority != *safe_authority_acc_info.key {
            return Err(LockupErrorCode::Unauthorized)?;
        }
        if !safe.initialized {
            return Err(LockupErrorCode::NotInitialized)?;
        }
        if safe_acc_info.owner != program_id {
            return Err(LockupErrorCode::InvalidAccountOwner)?;
        }
    }

    // Whitelist.
    {
        if safe.whitelist != *whitelist_acc_info.key {
            return Err(LockupErrorCode::InvalidWhitelist)?;
        }
        if whitelist_acc_info.owner != program_id {
            return Err(LockupErrorCode::InvalidAccountOwner)?;
        }
    }

    Ok(())
}

pub struct WhitelistGovRequest<'a> {
    pub program_id: &'a Pubkey,
    pub safe_authority_acc_info: &'a AccountInfo<'a>,
    pub safe_acc_info: &'a AccountInfo<'a>,
    pub whitelist_acc_info: &'a AccountInfo<'a>,
}

/// Access control on any instruction mutating an existing Vesting account.
pub fn vesting_gov(req: VestingGovRequest) -> Result<(), LockupError> {
    let VestingGovRequest {
        program_id,
        vesting,
        safe_acc_info,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
    } = req;

    // Beneficiary authorization.
    {
        if !vesting_acc_beneficiary_info.is_signer {
            return Err(LockupErrorCode::Unauthorized)?;
        }
    }

    // Safe account.
    let _ = safe(safe_acc_info, program_id)?;

    // Vesting account is valid.
    {
        if vesting_acc_info.owner != program_id {
            return Err(LockupErrorCode::InvalidAccount)?;
        }
        if !vesting.initialized {
            return Err(LockupErrorCode::NotInitialized)?;
        }
        if vesting.beneficiary != *vesting_acc_beneficiary_info.key {
            return Err(LockupErrorCode::Unauthorized)?;
        }
        if vesting.safe != *safe_acc_info.key {
            return Err(LockupErrorCode::WrongSafe)?;
        }
    }

    Ok(())
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

pub fn token(acc_info: &AccountInfo) -> Result<Account, LockupError> {
    if *acc_info.owner != spl_token::ID {
        return Err(LockupErrorCode::InvalidAccountOwner)?;
    }

    let token = Account::unpack(&acc_info.try_borrow_data()?)?;
    if token.state != spl_token::state::AccountState::Initialized {
        return Err(LockupErrorCode::NotInitialized)?;
    }

    Ok(token)
}

pub struct VestingGovRequest<'a, 'b> {
    pub vesting: &'b Vesting,
    pub program_id: &'a Pubkey,
    pub safe_acc_info: &'a AccountInfo<'a>,
    pub vesting_acc_info: &'a AccountInfo<'a>,
    pub vesting_acc_beneficiary_info: &'a AccountInfo<'a>,
}

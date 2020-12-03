use crate::common::access_control;
use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_lockup::accounts::{vault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    beneficiary: Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    nonce: u8,
) -> Result<(), LockupError> {
    msg!("handler: create_vesting");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { vault, clock_ts } = access_control(AccessControlRequest {
        program_id,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc_info,
        safe_acc_info,
        depositor_authority_acc_info,
        vault_acc_info,
        rent_acc_info,
        clock_acc_info,
        beneficiary,
        nonce,
    })?;

    Vesting::unpack_unchecked_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting: &mut Vesting| {
            state_transition(StateTransitionRequest {
                clock_ts,
                end_ts,
                period_count,
                deposit_amount,
                vesting,
                beneficiary,
                safe_acc_info,
                depositor_acc_info,
                depositor_authority_acc_info,
                token_program_acc_info,
                nonce,
                vault,
                vault_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, LockupError> {
    msg!("access-control: create_vesting");

    let AccessControlRequest {
        program_id,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc_info,
        safe_acc_info,
        vault_acc_info,
        depositor_authority_acc_info,
        rent_acc_info,
        clock_acc_info,
        beneficiary,
        nonce,
    } = req;

    // Depositor authorization.
    if !depositor_authority_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let _safe = access_control::safe(safe_acc_info, program_id)?;
    let rent = access_control::rent(rent_acc_info)?;
    let clock_ts = access_control::clock(&clock_acc_info)?.unix_timestamp;

    // Initialize checks.
    // Vesting account (uninitialized).
    {
        let mut data: &[u8] = &vesting_acc_info.try_borrow_data()?;
        let vesting = Vesting::unpack_unchecked(&mut data)?;

        if vesting_acc_info.owner != program_id {
            return Err(LockupErrorCode::NotOwnedByProgram)?;
        }
        if vesting.initialized {
            return Err(LockupErrorCode::AlreadyInitialized)?;
        }
        if !rent.is_exempt(
            vesting_acc_info.lamports(),
            vesting_acc_info.try_data_len()?,
        ) {
            return Err(LockupErrorCode::NotRentExempt)?;
        }
    }
    // Vesting schedule.
    {
        if end_ts <= clock_ts {
            return Err(LockupErrorCode::InvalidTimestamp)?;
        }
        if period_count == 0 {
            return Err(LockupErrorCode::InvalidPeriod)?;
        }
        if deposit_amount == 0 {
            return Err(LockupErrorCode::InvalidDepositAmount)?;
        }
    }
    // Vault owned by program.
    let vault = {
        let vault = access_control::token(vault_acc_info)?;
        let vault_authority = Pubkey::create_program_address(
            &vault::signer_seeds(safe_acc_info.key, &beneficiary, &nonce),
            program_id,
        )
        .map_err(|_| LockupErrorCode::InvalidVaultNonce)?;
        if vault.owner != vault_authority {
            return Err(LockupErrorCode::InvalidAccountOwner)?;
        }
        if vault.amount != 0 {
            return Err(LockupErrorCode::VaultAlreadyFunded)?;
        }
        vault
    };

    Ok(AccessControlResponse { vault, clock_ts })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    msg!("state-transition: create_vesting");

    let StateTransitionRequest {
        clock_ts,
        end_ts,
        period_count,
        deposit_amount,
        vesting,
        beneficiary,
        safe_acc_info,
        depositor_acc_info,
        vault,
        depositor_authority_acc_info,
        token_program_acc_info,
        vault_acc_info,
        nonce,
    } = req;

    // Initialize account.
    vesting.safe = safe_acc_info.key.clone();
    vesting.beneficiary = beneficiary;
    vesting.initialized = true;
    vesting.mint = vault.mint;
    vesting.vault = *vault_acc_info.key;
    vesting.period_count = period_count;
    vesting.start_balance = deposit_amount;
    vesting.end_ts = end_ts;
    vesting.start_ts = clock_ts;
    vesting.outstanding = deposit_amount;
    vesting.whitelist_owned = 0;
    vesting.grantor = *depositor_authority_acc_info.key;
    vesting.nonce = nonce;

    // Transfer funds to vault.
    invoke_token_transfer(
        depositor_acc_info,
        vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
        &[],
        deposit_amount,
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    vesting_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    beneficiary: Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    nonce: u8,
}

struct AccessControlResponse {
    vault: TokenAccount,
    clock_ts: i64,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vesting: &'c mut Vesting,
    vault: TokenAccount,
    beneficiary: Pubkey,
    clock_ts: i64,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    nonce: u8,
}

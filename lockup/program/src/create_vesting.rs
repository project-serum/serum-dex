use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{vault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::info;
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
    info!("handler: create_vesting");

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
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                clock_ts,
                end_ts,
                period_count,
                deposit_amount,
                vesting_acc,
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
    info!("access-control: create_vesting");

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
    let rent = access_control::rent(rent_acc_info)?;
    let _safe = access_control::safe(safe_acc_info, program_id)?;
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
    // Vault.
    let vault = {
        let vault_authority = Pubkey::create_program_address(
            &vault::signer_seeds(safe_acc_info.key, &beneficiary, &nonce),
            program_id,
        )
        .map_err(|_| LockupErrorCode::InvalidVaultNonce)?;
        let vault = access_control::token(vault_acc_info)?;
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
    info!("state-transition: create_vesting");

    let StateTransitionRequest {
        clock_ts,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc,
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
    {
        vesting_acc.safe = safe_acc_info.key.clone();
        vesting_acc.beneficiary = beneficiary;
        vesting_acc.initialized = true;
        vesting_acc.mint = vault.mint;
        vesting_acc.vault = *vault_acc_info.key;
        vesting_acc.period_count = period_count;
        vesting_acc.start_balance = deposit_amount;
        vesting_acc.end_ts = end_ts;
        vesting_acc.start_ts = clock_ts;
        vesting_acc.balance = deposit_amount;
        vesting_acc.whitelist_owned = 0;
        vesting_acc.grantor = *depositor_authority_acc_info.key;
        vesting_acc.nonce = nonce;
    }

    // Now transfer SPL funds from the depositor, to the
    // program-controlled vault.
    {
        let deposit_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            depositor_acc_info.key,
            vault_acc_info.key,
            depositor_authority_acc_info.key,
            &[],
            deposit_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &deposit_instruction,
            &[
                depositor_acc_info.clone(),
                depositor_authority_acc_info.clone(),
                vault_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[],
        )?;
    }

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    beneficiary: Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    vesting_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    nonce: u8,
}

struct AccessControlResponse {
    vault: TokenAccount,
    clock_ts: i64,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    clock_ts: i64,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    nonce: u8,
    vesting_acc: &'c mut Vesting,
    vault: TokenAccount,
    vault_acc_info: &'a AccountInfo<'b>,
    beneficiary: Pubkey,
    safe_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
}

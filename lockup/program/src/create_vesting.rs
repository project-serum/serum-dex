use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{TokenVault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_option::COption;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    vesting_acc_beneficiary: Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
) -> Result<(), LockupError> {
    info!("handler: create_vesting");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let nft_mint_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let clock_ts = access_control::clock(&clock_acc_info)?.unix_timestamp;

    access_control(AccessControlRequest {
        program_id,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc_info,
        safe_acc_info,
        depositor_authority_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        nft_mint_acc_info,
        rent_acc_info,
        clock_ts,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                clock_ts,
                end_ts,
                period_count,
                deposit_amount,
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc_info,
                nft_mint_acc_info,
                depositor_acc_info,
                vault_acc_info,
                depositor_authority_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), LockupError> {
    info!("access-control: create_vesting");

    let AccessControlRequest {
        program_id,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc_info,
        vault_authority_acc_info,
        safe_acc_info,
        nft_mint_acc_info,
        vault_acc_info,
        depositor_authority_acc_info,
        rent_acc_info,
        clock_ts,
    } = req;

    // Depositor authorization.
    if !depositor_authority_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let safe = access_control::safe(safe_acc_info, program_id)?;
    let _ = access_control::vault(
        vault_acc_info,
        vault_authority_acc_info,
        safe_acc_info,
        program_id,
    )?;

    // Initialize checks.
    {
        // Vesting account (uninitialized).
        {
            let vesting = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;

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
        // Vesting Mint.
        {
            let mint = access_control::mint(nft_mint_acc_info)?;
            let vault_authority = Pubkey::create_program_address(
                &TokenVault::signer_seeds(&safe_acc_info.key, &safe.nonce),
                program_id,
            )
            .map_err(|_| LockupErrorCode::InvalidVaultNonce)?;
            if mint.mint_authority != COption::Some(vault_authority) {
                return Err(LockupErrorCode::InvalidMintAuthority)?;
            }
            if mint.supply != 0 {
                return Err(LockupErrorCode::InvalidMintSupply)?;
            }
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), LockupError> {
    info!("state-transition: create_vesting");

    let StateTransitionRequest {
        clock_ts,
        end_ts,
        period_count,
        deposit_amount,
        vesting_acc,
        vesting_acc_beneficiary,
        safe_acc_info,
        depositor_acc_info,
        nft_mint_acc_info,
        vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
    } = req;

    // Initialize account.
    {
        vesting_acc.safe = safe_acc_info.key.clone();
        vesting_acc.beneficiary = vesting_acc_beneficiary;
        vesting_acc.initialized = true;
        vesting_acc.claimed = false;
        vesting_acc.period_count = period_count;
        vesting_acc.start_balance = deposit_amount;
        vesting_acc.end_ts = end_ts;
        vesting_acc.start_ts = clock_ts;
        vesting_acc.balance = deposit_amount;
        vesting_acc.locked_nft_mint = *nft_mint_acc_info.key;
        vesting_acc.whitelist_owned = 0;
    }

    // Now transfer SPL funds from the depositor, to the
    // program-controlled vault.
    {
        info!("invoke SPL token transfer");
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

    info!("state-transition: complete");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    nft_mint_acc_info: &'a AccountInfo<'a>,
    vault_authority_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    clock_ts: i64,
}

struct StateTransitionRequest<'a, 'b> {
    clock_ts: i64,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
    vesting_acc: &'b mut Vesting,
    vesting_acc_beneficiary: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    nft_mint_acc_info: &'a AccountInfo<'a>,
}

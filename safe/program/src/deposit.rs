use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault, Vesting};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    vesting_acc_beneficiary: Pubkey,
    end_slot: u64,
    period_count: u64,
    deposit_amount: u64,
) -> Result<(), SafeError> {
    info!("handler: deposit");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let clock_slot = Clock::from_account_info(clock_acc_info)?.slot;

    access_control(AccessControlRequest {
        program_id,
        end_slot,
        period_count,
        deposit_amount,
        vesting_acc_info,
        safe_acc_info,
        depositor_acc_info,
        depositor_authority_acc_info,
        safe_vault_acc_info,
        token_program_acc_info,
        rent_acc_info,
        clock_acc_info,
        clock_slot,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                clock_slot,
                end_slot,
                period_count,
                deposit_amount,
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc_info,
                depositor_acc_info,
                safe_vault_acc_info,
                depositor_authority_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: deposit");

    let AccessControlRequest {
        program_id,
        end_slot,
        period_count,
        deposit_amount,
        vesting_acc_info,
        safe_acc_info,
        depositor_acc_info,
        safe_vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
        rent_acc_info,
        clock_acc_info,
        clock_slot,
    } = req;

    // Depositor authorization.
    {
        if !depositor_authority_acc_info.is_signer {
            return Err(SafeErrorCode::Unauthorized)?;
        }
    }

    // Safe.
    let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
    {
        if !safe.initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
    }

    // Vault.
    {
        let safe_vault =
            spl_token::state::Account::unpack(&safe_vault_acc_info.try_borrow_data()?)?;

        if *safe_vault_acc_info.owner != spl_token::ID {
            return Err(SafeErrorCode::InvalidVault)?;
        }
        if safe_vault.state != spl_token::state::AccountState::Initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
        let safe_vault_authority = Pubkey::create_program_address(
            &TokenVault::signer_seeds(safe_acc_info.key, &safe.nonce),
            program_id,
        )
        .map_err(|_| SafeErrorCode::InvalidVault)?;
        if safe_vault.owner != safe_vault_authority {
            return Err(SafeErrorCode::InvalidVault)?;
        }
    }

    // Vesting.
    {
        if vesting_acc_info.owner != program_id {
            return Err(SafeErrorCode::NotOwnedByProgram)?;
        }
        let vesting = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;
        if vesting.initialized {
            return Err(SafeErrorCode::AlreadyInitialized)?;
        }
        let rent = Rent::from_account_info(rent_acc_info)?;
        if !rent.is_exempt(
            vesting_acc_info.lamports(),
            vesting_acc_info.try_data_len()?,
        ) {
            return Err(SafeErrorCode::NotRentExempt)?;
        }
    }

    // Token program.
    {
        if *token_program_acc_info.key != spl_token::ID {
            return Err(SafeErrorCode::InvalidTokenProgram)?;
        }
    }

    // Rent sysvar.
    {
        if *rent_acc_info.key != solana_sdk::sysvar::rent::id() {
            return Err(SafeErrorCode::InvalidRentSysvar)?;
        }
    }

    // Vesting schedule.
    {
        if *clock_acc_info.key != solana_sdk::sysvar::clock::id() {
            return Err(SafeErrorCode::InvalidClock)?;
        }
        if end_slot <= clock_slot {
            return Err(SafeErrorCode::InvalidSlot)?;
        }
        if period_count == 0 {
            return Err(SafeErrorCode::InvalidPeriod)?;
        }
        if deposit_amount == 0 {
            return Err(SafeErrorCode::InvalidDepositAmount)?;
        }
    }

    // Depositor.
    {
        let depositor = spl_token::state::Account::unpack(&depositor_acc_info.try_borrow_data()?)?;
        if safe.mint != depositor.mint {
            return Err(SafeErrorCode::WrongCoinMint)?;
        }
        // Let the spl token program handle the rest of the depositor.
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: deposit");

    let StateTransitionRequest {
        clock_slot,
        end_slot,
        period_count,
        deposit_amount,
        vesting_acc,
        vesting_acc_beneficiary,
        safe_acc_info,
        depositor_acc_info,
        safe_vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
    } = req;

    // Initialize account.
    {
        vesting_acc.safe = safe_acc_info.key.clone();
        vesting_acc.beneficiary = vesting_acc_beneficiary;
        vesting_acc.initialized = true;
        vesting_acc.locked_outstanding = 0;
        vesting_acc.period_count = period_count;
        vesting_acc.start_balance = deposit_amount;
        vesting_acc.end_slot = end_slot;
        vesting_acc.start_slot = clock_slot;
        vesting_acc.balance = deposit_amount;
    }

    // Now transfer SPL funds from the depositor, to the
    // program-controlled vault.
    {
        info!("invoke SPL token transfer");

        let deposit_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            depositor_acc_info.key,
            safe_vault_acc_info.key,
            depositor_authority_acc_info.key,
            &[],
            deposit_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &deposit_instruction,
            &[
                depositor_acc_info.clone(),
                depositor_authority_acc_info.clone(),
                safe_vault_acc_info.clone(),
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
    end_slot: u64,
    period_count: u64,
    deposit_amount: u64,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    clock_acc_info: &'a AccountInfo<'a>,
    clock_slot: u64,
}

struct StateTransitionRequest<'a, 'b> {
    clock_slot: u64,
    end_slot: u64,
    period_count: u64,
    deposit_amount: u64,
    vesting_acc: &'b mut Vesting,
    vesting_acc_beneficiary: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

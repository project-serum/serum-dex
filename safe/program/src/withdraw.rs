use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault, Vesting};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack as TokenPack;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
) -> Result<(), SafeError> {
    info!("handler: withdraw");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_beneficiary_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let beneficiary_token_acc_info = next_account_info(acc_infos)?;
    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        amount,
        vesting_acc_beneficiary_info,
        vesting_acc_info,
        safe_vault_acc_info,
        safe_acc_info,
        token_program_acc_info,
        clock_acc_info,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                amount,
                vesting_acc,
                safe_vault_acc_info,
                safe_vault_authority_acc_info,
                beneficiary_token_acc_info,
                safe_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: withdraw");

    let AccessControlRequest {
        program_id,
        amount,
        vesting_acc_beneficiary_info,
        vesting_acc_info,
        safe_vault_acc_info,
        safe_acc_info,
        token_program_acc_info,
        clock_acc_info,
    } = req;

    // Beneficiary authorization.
    {
        if !vesting_acc_beneficiary_info.is_signer {
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
            return Err(SafeErrorCode::InvalidVault)?;
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
        let vesting = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;

        if vesting_acc_info.owner != program_id {
            return Err(SafeErrorCode::InvalidAccount)?;
        }
        if !vesting.initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
        // Match the signing beneficiary to this account.
        if vesting.beneficiary != *vesting_acc_beneficiary_info.key {
            return Err(SafeErrorCode::Unauthorized)?;
        }
        // Match the vesting account to the safe.
        if vesting.safe != *safe_acc_info.key {
            return Err(SafeErrorCode::WrongSafe)?;
        }
        // Do we have sufficient balance?
        let clock = Clock::from_account_info(clock_acc_info)?;
        if amount > vesting.available_for_withdrawal(clock.slot) {
            return Err(SafeErrorCode::InsufficientBalance)?;
        }
    }

    // Token program.
    {
        if *token_program_acc_info.key != spl_token::ID {
            return Err(SafeErrorCode::InvalidTokenProgram)?;
        }
    }

    // Beneficiary token account.
    {
        // Allow the SPL token program to handle this.
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: withdraw");

    let StateTransitionRequest {
        vesting_acc,
        amount,
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        beneficiary_token_acc_info,
        safe_acc_info,
        token_program_acc_info,
    } = req;

    // Remove the withdrawn token from the vesting account.
    {
        vesting_acc.deduct(amount);
    }

    // Withdraw token from vault to the user address.
    {
        info!("invoking withdrawal token transfer");

        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            safe_vault_acc_info.key,
            beneficiary_token_acc_info.key,
            &safe_vault_authority_acc_info.key,
            &[],
            amount,
        )?;

        let data = safe_acc_info.try_borrow_data()?;
        let nonce = data[data.len() - 1];
        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(
            &withdraw_instruction,
            &[
                safe_vault_acc_info.clone(),
                beneficiary_token_acc_info.clone(),
                safe_vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    amount: u64,
    vesting_acc_beneficiary_info: &'a AccountInfo<'a>,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    clock_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    amount: u64,
    vesting_acc: &'b mut Vesting,
    safe_acc_info: &'a AccountInfo<'a>,
    beneficiary_token_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

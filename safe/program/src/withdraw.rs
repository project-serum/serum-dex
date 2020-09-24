use serum_common::pack::Pack;
use serum_safe::accounts::{TokenVault, Vesting};
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
    let beneficiary_spl_acc_info = next_account_info(acc_infos)?;
    let safe_spl_vault_acc_info = next_account_info(acc_infos)?;
    let safe_spl_vault_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let spl_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        amount,
        vesting_acc_beneficiary_info,
        vesting_acc_info,
        safe_spl_vault_acc_info,
        safe_acc_info,
        spl_program_acc_info,
        clock_acc_info,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                amount,
                vesting_acc,
                safe_spl_vault_acc_info,
                safe_spl_vault_authority_acc_info,
                beneficiary_spl_acc_info,
                safe_acc_info,
                spl_program_acc_info,
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
        safe_spl_vault_acc_info,
        safe_acc_info,
        spl_program_acc_info,
        clock_acc_info,
    } = req;
    assert_eq!(*spl_program_acc_info.key, spl_token::ID);

    if vesting_acc_info.owner != program_id {
        return Err(SafeErrorCode::InvalidAccount)?;
    }
    if !vesting_acc_beneficiary_info.is_signer {
        return Err(SafeErrorCode::Unauthorized)?;
    }
    let vesting_acc = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;
    if vesting_acc.beneficiary != *vesting_acc_beneficiary_info.key {
        return Err(SafeErrorCode::Unauthorized)?;
    }
    let clock = Clock::from_account_info(clock_acc_info)?;
    if amount > vesting_acc.available_for_withdrawal(clock.slot) {
        return Err(SafeErrorCode::InsufficientBalance)?;
    }

    // Validate the vault spl account.
    {
        let spl_vault_data = safe_spl_vault_acc_info.try_borrow_data()?;

        assert_eq!(*safe_spl_vault_acc_info.owner, spl_token::ID);
        assert_eq!(spl_vault_data.len(), spl_token::state::Account::LEN);

        // AccountState must be initialized.
        if spl_vault_data[0x6c] != 1u8 {
            return Err(SafeErrorCode::WrongVault)?;
        }
        // The SPL account owner must be hte program derived address.
        let expected_owner = {
            let data = safe_acc_info.try_borrow_data()?;
            let nonce = data[data.len() - 1];
            let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

            Pubkey::create_program_address(&signer_seeds, program_id)
                .expect("safe initialized with invalid nonce")
        };
        let owner = Pubkey::new(&spl_vault_data[32..64]);
        if owner != expected_owner {
            return Err(SafeErrorCode::WrongVault)?;
        }
    }
    // todo: check beneficiary account is initialized
    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: withdraw");

    let StateTransitionRequest {
        vesting_acc,
        amount,
        safe_spl_vault_acc_info,
        safe_spl_vault_authority_acc_info,
        beneficiary_spl_acc_info,
        safe_acc_info,
        spl_program_acc_info,
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
            safe_spl_vault_acc_info.key,
            beneficiary_spl_acc_info.key,
            &safe_spl_vault_authority_acc_info.key,
            &[],
            amount,
        )?;

        let data = safe_acc_info.try_borrow_data()?;
        let nonce = data[data.len() - 1];
        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(
            &withdraw_instruction,
            &[
                safe_spl_vault_acc_info.clone(),
                beneficiary_spl_acc_info.clone(),
                safe_spl_vault_authority_acc_info.clone(),
                spl_program_acc_info.clone(),
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
    clock_acc_info: &'a AccountInfo<'a>,
    safe_spl_vault_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    spl_program_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    amount: u64,
    vesting_acc: &'b mut Vesting,
    safe_spl_vault_acc_info: &'a AccountInfo<'a>,
    beneficiary_spl_acc_info: &'a AccountInfo<'a>,
    safe_spl_vault_authority_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    spl_program_acc_info: &'a AccountInfo<'a>,
}

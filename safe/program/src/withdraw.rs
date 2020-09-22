use serum_safe::accounts::{SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
) -> Result<(), SafeError> {
    info!("HANDLER: withdraw_srm");

    let account_info_iter = &mut accounts.iter();

    let vesting_account_beneficiary_info = next_account_info(account_info_iter)?;
    let vesting_account_info = next_account_info(account_info_iter)?;
    let beneficiary_spl_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_authority_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let spl_program_account_info = next_account_info(account_info_iter)?;
    let clock = Clock::from_account_info(next_account_info(account_info_iter)?)?;

    VestingAccount::unpack_mut(
        &mut vesting_account_info.try_borrow_mut_data()?,
        &mut |vesting_account: &mut VestingAccount| {
            access_control(AccessControlRequest {
                program_id,
                vesting_account_beneficiary_info: &vesting_account_beneficiary_info,
                vesting_account_info,
                vesting_account,
                amount,
                clock: &clock,
                safe_spl_vault_account_info,
                safe_account_info,
                spl_program_account_info,
            })?;

            state_transition(StateTransitionRequest {
                vesting_account,
                amount,
                safe_spl_vault_account_info,
                safe_spl_vault_authority_account_info,
                beneficiary_spl_account_info,
                safe_account_info,
                spl_program_account_info,
            })?;

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: withdraw");

    let AccessControlRequest {
        program_id,
        vesting_account_beneficiary_info,
        vesting_account_info,
        vesting_account,
        amount,
        clock,
        safe_spl_vault_account_info,
        safe_account_info,
        spl_program_account_info,
    } = req;
    assert_eq!(*spl_program_account_info.key, spl_token::ID);

    if vesting_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::InvalidAccount));
    }
    if !vesting_account_beneficiary_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if vesting_account.beneficiary != *vesting_account_beneficiary_info.key {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if amount > vesting_account.available_for_withdrawal(clock.slot) {
        return Err(SafeError::ErrorCode(SafeErrorCode::InsufficientBalance));
    }

    // Validate the vault spl account.
    {
        let spl_vault_data = safe_spl_vault_account_info.try_borrow_data()?;

        assert_eq!(*safe_spl_vault_account_info.owner, spl_token::ID);
        assert_eq!(spl_vault_data.len(), spl_token::state::Account::LEN);

        // AccountState must be initialized.
        if spl_vault_data[0x6c] != 1u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault));
        }
        // The SPL account owner must be hte program derived address.
        let expected_owner = {
            let data = safe_account_info.try_borrow_data()?;
            let nonce = &[data[data.len() - 1]];
            let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

            Pubkey::create_program_address(&signer_seeds, program_id)
                .expect("safe initialized with invalid nonce")
        };
        let owner = Pubkey::new(&spl_vault_data[32..64]);
        if owner != expected_owner {
            return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault));
        }
    }
    // todo: check beneficiary account is initialized
    info!("access-control: success");
    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    vesting_account_beneficiary_info: &'a AccountInfo<'a>,
    vesting_account_info: &'a AccountInfo<'a>,
    vesting_account: &'b VestingAccount,
    amount: u64,
    clock: &'b Clock,
    safe_spl_vault_account_info: &'a AccountInfo<'a>,
    safe_account_info: &'a AccountInfo<'a>,
    spl_program_account_info: &'a AccountInfo<'a>,
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: withdraw");

    let StateTransitionRequest {
        vesting_account,
        amount,
        safe_spl_vault_account_info,
        safe_spl_vault_authority_account_info,
        beneficiary_spl_account_info,
        safe_account_info,
        spl_program_account_info,
    } = req;

    vesting_account.deduct(amount);

    info!("invoking withdrawal token transfer");

    let withdraw_instruction = spl_token::instruction::transfer(
        &spl_token::ID,
        safe_spl_vault_account_info.key,
        beneficiary_spl_account_info.key,
        &safe_spl_vault_authority_account_info.key,
        &[],
        amount,
    )?;

    let data = safe_account_info.try_borrow_data()?;
    let nonce = &[data[data.len() - 1]];
    let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

    solana_sdk::program::invoke_signed(
        &withdraw_instruction,
        &[
            safe_spl_vault_account_info.clone(),
            beneficiary_spl_account_info.clone(),
            safe_spl_vault_authority_account_info.clone(),
            spl_program_account_info.clone(),
        ],
        &[&signer_seeds],
    )?;

    info!("withdrawal token transfer complete");
    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a, 'b> {
    vesting_account: &'b mut VestingAccount,
    amount: u64,
    safe_spl_vault_account_info: &'a AccountInfo<'a>,
    beneficiary_spl_account_info: &'a AccountInfo<'a>,
    safe_spl_vault_authority_account_info: &'a AccountInfo<'a>,
    safe_account_info: &'a AccountInfo<'a>,
    spl_program_account_info: &'a AccountInfo<'a>,
}

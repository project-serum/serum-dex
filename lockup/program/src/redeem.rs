use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{vault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> Result<(), LockupError> {
    info!("handler: redeem");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let beneficiary_token_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        amount,
        beneficiary_acc_info,
        vesting_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        safe_acc_info,
        clock_acc_info,
    })?;

    Vesting::unpack_unchecked_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting: &mut Vesting| {
            state_transition(StateTransitionRequest {
                amount,
                vesting,
                vault_acc_info,
                vault_authority_acc_info,
                beneficiary_token_acc_info,
                safe_acc_info,
                token_program_acc_info,
                beneficiary_acc_info,
            })
            .map_err(Into::into)
        },
    )
    .map_err(|e| LockupError::ProgramError(e))
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: redeem");

    let AccessControlRequest {
        program_id,
        amount,
        beneficiary_acc_info,
        vesting_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        safe_acc_info,
        clock_acc_info,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let _ = access_control::safe(safe_acc_info, program_id)?;
    let vesting = access_control::vesting(
        program_id,
        safe_acc_info.key,
        vesting_acc_info,
        beneficiary_acc_info,
    )?;
    let _ = access_control::vault(
        vault_acc_info,
        vault_authority_acc_info,
        vesting_acc_info,
        beneficiary_acc_info,
        safe_acc_info,
        program_id,
    )?;

    // Redemption checks.
    {
        let clock = access_control::clock(clock_acc_info)?;
        if amount == 0 || amount > vesting.available_for_withdrawal(clock.unix_timestamp) {
            return Err(LockupErrorCode::InsufficientWithdrawalBalance)?;
        }
    }

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: redeem");

    let StateTransitionRequest {
        vesting,
        amount,
        vault_acc_info,
        vault_authority_acc_info,
        beneficiary_token_acc_info,
        safe_acc_info,
        token_program_acc_info,
        beneficiary_acc_info,
    } = req;

    // Remove the withdrawn token from the vesting account.
    {
        vesting.deduct(amount);
    }

    // Transfer token from the vault to the user address.
    {
        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            beneficiary_token_acc_info.key,
            &vault_authority_acc_info.key,
            &[],
            amount,
        )?;

        let signer_seeds =
            vault::signer_seeds(safe_acc_info.key, beneficiary_acc_info.key, &vesting.nonce);

        solana_sdk::program::invoke_signed(
            &withdraw_instruction,
            &[
                vault_acc_info.clone(),
                beneficiary_token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    amount: u64,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    vesting_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    amount: u64,
    vesting: &'c mut Vesting,
    safe_acc_info: &'a AccountInfo<'b>,
    beneficiary_token_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
}

use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, TokenVault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
) -> Result<(), LockupError> {
    info!("handler: redeem");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_beneficiary_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let beneficiary_token_acc_info = next_account_info(acc_infos)?;
    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let nft_token_acc_info = next_account_info(acc_infos)?;
    let nft_mint_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        amount,
        vesting_acc_beneficiary_info,
        vesting_acc_info,
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        safe_acc_info,
        nft_token_acc_info,
        nft_mint_acc_info,
        clock_acc_info,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                amount,
                vesting_acc,
                accounts,
                safe_vault_acc_info,
                safe_vault_authority_acc_info,
                beneficiary_token_acc_info,
                safe_acc_info,
                token_program_acc_info,
                nft_token_acc_info,
                nft_mint_acc_info,
            })
            .map_err(Into::into)
        },
    )
    .map_err(|e| LockupError::ProgramError(e))
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), LockupError> {
    info!("access-control: redeem");

    let AccessControlRequest {
        program_id,
        amount,
        vesting_acc_beneficiary_info,
        vesting_acc_info,
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        safe_acc_info,
        nft_token_acc_info,
        nft_mint_acc_info,
        clock_acc_info,
    } = req;

    // Beneficiary authorization.
    if !vesting_acc_beneficiary_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let _ = access_control::safe(safe_acc_info, program_id)?;
    let _ = access_control::vault(
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        safe_acc_info,
        program_id,
    )?;
    let vesting = access_control::vesting(
        program_id,
        safe_acc_info.key,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
    )?;
    let _ = access_control::locked_token(
        nft_token_acc_info,
        nft_mint_acc_info,
        safe_vault_authority_acc_info.key,
        &vesting,
    )?;

    // Redemption checks.
    {
        let clock = access_control::clock(clock_acc_info)?;

        if !vesting.claimed {
            return Err(LockupErrorCode::NotYetClaimed)?;
        }
        if amount > vesting.available_for_withdrawal(clock.unix_timestamp) {
            return Err(LockupErrorCode::InsufficientWithdrawalBalance)?;
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), LockupError> {
    info!("state-transition: redeem");

    let StateTransitionRequest {
        vesting_acc,
        amount,
        accounts,
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        beneficiary_token_acc_info,
        safe_acc_info,
        token_program_acc_info,
        nft_token_acc_info,
        nft_mint_acc_info,
    } = req;

    // Remove the withdrawn token from the vesting account.
    {
        vesting_acc.deduct(amount);
    }

    // Burn the NFT.
    {
        info!("burning token receipts");
        let burn_instruction = spl_token::instruction::burn(
            &spl_token::ID,
            nft_token_acc_info.key,
            nft_mint_acc_info.key,
            &vesting_acc.beneficiary,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(&burn_instruction, &accounts[..], &[])?;
    }

    // Transfer token from the vault to the user address.
    {
        info!("invoking token transfer");
        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            safe_vault_acc_info.key,
            beneficiary_token_acc_info.key,
            &safe_vault_authority_acc_info.key,
            &[],
            amount,
        )?;

        let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &safe.nonce);

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
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    nft_token_acc_info: &'a AccountInfo<'a>,
    nft_mint_acc_info: &'a AccountInfo<'a>,
    clock_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    amount: u64,
    vesting_acc: &'b mut Vesting,
    accounts: &'a [AccountInfo<'a>],
    safe_acc_info: &'a AccountInfo<'a>,
    beneficiary_token_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    nft_token_acc_info: &'a AccountInfo<'a>,
    nft_mint_acc_info: &'a AccountInfo<'a>,
}

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
) -> Result<(), LockupError> {
    info!("handler: claim");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_beneficiary_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let _token_prog_acc_info = next_account_info(acc_infos)?;
    let mint_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_vault_authority_acc_info,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        mint_acc_info,
        token_acc_info,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
            state_transition(StateTransitionRequest {
                accounts,
                vesting_acc,
                safe_acc_info,
                safe_vault_authority_acc_info,
                mint_acc_info,
                token_acc_info,
                nonce: safe.nonce,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), LockupError> {
    info!("access-control: claim");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_vault_authority_acc_info,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        mint_acc_info,
        token_acc_info,
    } = req;

    // Beneficiary authorization.
    if !vesting_acc_beneficiary_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let safe = access_control::safe(safe_acc_info, program_id)?;
    let _ = access_control::vault_authority(
        safe_vault_authority_acc_info,
        &safe_acc_info.key,
        &safe,
        program_id,
    )?;
    let vesting = access_control::vesting(
        program_id,
        safe_acc_info.key,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
    )?;
    let _ = access_control::locked_token(
        token_acc_info,
        mint_acc_info,
        safe_vault_authority_acc_info.key,
        &vesting,
    )?;

    // Claim checks.
    if vesting.claimed {
        return Err(LockupErrorCode::AlreadyClaimed)?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), LockupError> {
    info!("state-transition: claim");

    let StateTransitionRequest {
        accounts,
        safe_acc_info,
        safe_vault_authority_acc_info,
        mint_acc_info,
        token_acc_info,
        vesting_acc,
        nonce,
    } = req;

    // Mint all the tokens associated with the locked token receipt. They
    // can't actualy be redeemed for anything without the beneficiary signing
    // off.
    {
        info!("invoke: spl_token::instruction::mint_to");

        let mint_to_instr = spl_token::instruction::mint_to(
            &spl_token::ID,
            mint_acc_info.key,
            token_acc_info.key,
            safe_vault_authority_acc_info.key,
            &[],
            vesting_acc.start_balance,
        )?;

        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(&mint_to_instr, &accounts[..], &[&signer_seeds])?;
    }

    vesting_acc.claimed = true;
    vesting_acc.locked_nft_token = *token_acc_info.key;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    vesting_acc_info: &'a AccountInfo<'a>,
    vesting_acc_beneficiary_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    token_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    accounts: &'a [AccountInfo<'a>],
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    token_acc_info: &'a AccountInfo<'a>,
    vesting_acc: &'b mut Vesting,
    nonce: u8,
}

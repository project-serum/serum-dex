use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack as TokenPack;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
) -> Result<(), SafeError> {
    info!("handler: migrate");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let receiver_spl_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_authority_acc_info,
        token_program_acc_info,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe_acc: &mut Safe| {
            let safe_vault =
                spl_token::state::Account::unpack(&safe_vault_acc_info.try_borrow_data()?)?;
            state_transition(StateTransitionRequest {
                safe_vault_amount: safe_vault.amount,
                safe_acc,
                safe_acc_info,
                safe_vault_acc_info,
                safe_vault_authority_acc_info,
                receiver_spl_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: migrate");

    let AccessControlRequest {
        program_id,
        safe_acc_info,
        safe_authority_acc_info,
        token_program_acc_info,
    } = req;

    // Safe authority authorization.
    {
        if !safe_authority_acc_info.is_signer {
            return Err(SafeErrorCode::Unauthorized)?;
        }
    }

    // Safe.
    {
        let safe_acc = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
        // Match the safe to the authority.
        if safe_acc.authority != *safe_authority_acc_info.key {
            return Err(SafeErrorCode::Unauthorized)?;
        }
        if !safe_acc.initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
        if safe_acc_info.owner != program_id {
            return Err(SafeErrorCode::InvalidAccountOwner)?;
        }
    }

    // Token program.
    {
        if *token_program_acc_info.key != spl_token::ID {
            return Err(SafeErrorCode::InvalidTokenProgram)?;
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: migrate");

    let StateTransitionRequest {
        safe_acc,
        safe_acc_info,
        safe_vault_acc_info,
        safe_vault_authority_acc_info,
        safe_vault_amount,
        receiver_spl_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer all tokens to the new account.
    {
        info!("invoking migration token transfer");

        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            safe_vault_acc_info.key,
            receiver_spl_acc_info.key,
            &safe_vault_authority_acc_info.key,
            &[],
            safe_vault_amount,
        )?;

        let seeds = TokenVault::signer_seeds(safe_acc_info.key, &safe_acc.nonce);
        let accs = vec![
            safe_vault_acc_info.clone(),
            receiver_spl_acc_info.clone(),
            safe_vault_authority_acc_info.clone(),
            token_program_acc_info.clone(),
        ];
        solana_sdk::program::invoke_signed(&withdraw_instruction, &accs, &[&seeds])?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    safe_acc: &'b mut Safe,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_amount: u64,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    receiver_spl_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

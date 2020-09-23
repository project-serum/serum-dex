use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack as TokenPack;

pub fn handler<'a>(_program_id: &Pubkey, accounts: &'a [AccountInfo<'a>]) -> Result<(), SafeError> {
    info!("handler: migrate");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_spl_vault_acc_info = next_account_info(acc_infos)?;
    let safe_spl_vault_authority_acc_info = next_account_info(acc_infos)?;
    let receiver_spl_acc_info = next_account_info(acc_infos)?;
    let spl_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        safe_acc_info,
        safe_authority_acc_info,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe_acc: &mut Safe| {
            let safe_spl_vault =
                spl_token::state::Account::unpack(&safe_spl_vault_acc_info.try_borrow_data()?)?;
            state_transition(StateTransitionRequest {
                safe_acc,
                safe_acc_info,
                safe_spl_vault,
                safe_spl_vault_acc_info,
                safe_spl_vault_authority_acc_info,
                receiver_spl_acc_info,
                spl_program_acc_info,
            })?;

            Ok(())
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: migrate");

    let AccessControlRequest {
        safe_acc_info,
        safe_authority_acc_info,
    } = req;

    let safe_acc = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;

    if !safe_acc.initialized {}
    if !safe_authority_acc_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if safe_acc.authority != *safe_authority_acc_info.key {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: migrate");

    let StateTransitionRequest {
        safe_acc,
        safe_acc_info,
        safe_spl_vault,
        safe_spl_vault_acc_info,
        safe_spl_vault_authority_acc_info,
        receiver_spl_acc_info,
        spl_program_acc_info,
    } = req;

    // Transfer all tokens to the new account.
    {
        info!("invoking migration token transfer");

        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            safe_spl_vault_acc_info.key,
            receiver_spl_acc_info.key,
            &safe_spl_vault_authority_acc_info.key,
            &[],
            safe_spl_vault.amount,
        )?;

        let seeds = TokenVault::signer_seeds(safe_acc_info.key, &safe_acc.nonce);
        let accs = vec![
            safe_spl_vault_acc_info.clone(),
            receiver_spl_acc_info.clone(),
            safe_spl_vault_authority_acc_info.clone(),
            spl_program_acc_info.clone(),
        ];
        solana_sdk::program::invoke_signed(&withdraw_instruction, &accs, &[&seeds])?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    safe_acc_info: &'a AccountInfo<'a>,
    safe_authority_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    safe_acc: &'b mut Safe,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_spl_vault: spl_token::state::Account,
    safe_spl_vault_acc_info: &'a AccountInfo<'a>,
    safe_spl_vault_authority_acc_info: &'a AccountInfo<'a>,
    receiver_spl_acc_info: &'a AccountInfo<'a>,
    spl_program_acc_info: &'a AccountInfo<'a>,
}

use serum_safe::accounts::{SafeAccount, SrmVault};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack;

pub fn handler<'a>(_program_id: &Pubkey, accounts: &'a [AccountInfo<'a>]) -> Result<(), SafeError> {
    info!("handler: migrate");

    let account_info_iter = &mut accounts.iter();

    let safe_authority_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_authority_account_info = next_account_info(account_info_iter)?;
    let receiver_spl_account_info = next_account_info(account_info_iter)?;
    let spl_program_account_info = next_account_info(account_info_iter)?;

    let mut safe_account_data = safe_account_info.try_borrow_mut_data()?;

    SafeAccount::unpack_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            let safe_spl_vault =
                spl_token::state::Account::unpack(&safe_spl_vault_account_info.try_borrow_data()?)?;

            access_control(AccessControlRequest {
                safe_authority_account_info,
                safe_account_authority: &safe_account.authority,
            })?;

            state_transition(StateTransitionRequest {
                safe_account,
                safe_account_info,
                safe_spl_vault,
                safe_spl_vault_account_info,
                safe_spl_vault_authority_account_info,
                receiver_spl_account_info,
                spl_program_account_info,
            })?;

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: migrate");
    let AccessControlRequest {
        safe_authority_account_info,
        safe_account_authority,
    } = req;
    if !safe_authority_account_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if safe_account_authority != safe_authority_account_info.key {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    // todo
    info!("access-control: success");
    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    safe_authority_account_info: &'a AccountInfo<'a>,
    safe_account_authority: &'b Pubkey,
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: migrate");

    let StateTransitionRequest {
        safe_account,
        safe_account_info,
        safe_spl_vault,
        safe_spl_vault_account_info,
        safe_spl_vault_authority_account_info,
        receiver_spl_account_info,
        spl_program_account_info,
    } = req;

    info!("invoking migration token transfer");

    let withdraw_instruction = spl_token::instruction::transfer(
        &spl_token::ID,
        safe_spl_vault_account_info.key,
        receiver_spl_account_info.key,
        &safe_spl_vault_authority_account_info.key,
        &[],
        safe_spl_vault.amount,
    )?;

    let nonce = &[safe_account.nonce];
    let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);
    let accounts = vec![
        safe_spl_vault_account_info.clone(),
        receiver_spl_account_info.clone(),
        safe_spl_vault_authority_account_info.clone(),
        spl_program_account_info.clone(),
    ];
    solana_sdk::program::invoke_signed(&withdraw_instruction, &accounts, &[&signer_seeds])?;

    info!("migration token transfer complete");

    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a, 'b> {
    safe_account: &'b mut SafeAccount,
    safe_account_info: &'a AccountInfo<'a>,
    safe_spl_vault: spl_token::state::Account,
    safe_spl_vault_account_info: &'a AccountInfo<'a>,
    safe_spl_vault_authority_account_info: &'a AccountInfo<'a>,
    receiver_spl_account_info: &'a AccountInfo<'a>,
    spl_program_account_info: &'a AccountInfo<'a>,
}

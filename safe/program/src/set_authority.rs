use serum_safe::accounts::SafeAccount;
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack;

pub fn handler<'a>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    new_authority: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: set_authority");

    let account_info_iter = &mut accounts.iter();

    let safe_authority_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;

    let mut safe_account_data = safe_account_info.try_borrow_mut_data()?;

    SafeAccount::unpack_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            access_control(AccessControlRequest {
                safe_authority_account_info,
                safe_account_authority: &safe_account.authority,
            })?;

            state_transition(StateTransitionRequest {
                safe_account,
                new_authority,
            })?;

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: set-authority");

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

    info!("access-control: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    safe_authority_account_info: &'a AccountInfo<'a>,
    safe_account_authority: &'b Pubkey,
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), SafeError> {
    info!("state-transition: set-authority");

    let StateTransitionRequest {
        safe_account,
        new_authority,
    } = req;

    safe_account.authority = new_authority;

    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a> {
    safe_account: &'a mut SafeAccount,
    new_authority: Pubkey,
}

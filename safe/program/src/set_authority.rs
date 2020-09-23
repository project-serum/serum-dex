use serum_common::pack::Pack;
use serum_safe::accounts::Safe;
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    new_authority: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: set_authority");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        safe_acc_info,
        safe_authority_acc_info,
    })?;

    Safe::unpack_mut(
        &mut safe_acc_info.try_borrow_mut_data()?,
        &mut |safe_acc: &mut Safe| {
            state_transition(StateTransitionRequest {
                safe_acc,
                new_authority,
            })?;

            Ok(())
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: set-authority");

    let AccessControlRequest {
        safe_acc_info,
        safe_authority_acc_info,
    } = req;

    let safe_acc = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
    if !safe_authority_acc_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if safe_acc.authority != *safe_authority_acc_info.key {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }

    info!("access-control: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    safe_acc_info: &'a AccountInfo<'a>,
    safe_authority_acc_info: &'a AccountInfo<'a>,
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), SafeError> {
    info!("state-transition: set-authority");

    let StateTransitionRequest {
        safe_acc,
        new_authority,
    } = req;

    safe_acc.authority = new_authority;

    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a> {
    safe_acc: &'a mut Safe,
    new_authority: Pubkey,
}

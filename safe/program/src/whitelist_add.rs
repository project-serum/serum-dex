use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, Whitelist};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    program_id_to_add: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: whitelist_add");

    let acc_infos = &mut accounts.iter();

    let safe_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let whitelist_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
    })?;

    Whitelist::unpack_mut(
        &mut whitelist_acc_info.try_borrow_mut_data()?,
        &mut |whitelist: &mut Whitelist| {
            state_transition(StateTransitionRequest {
                whitelist,
                program_id_to_add,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), SafeError> {
    info!("access-control: whitelist_add");

    let AccessControlRequest {
        safe_authority_acc_info,
        safe_acc_info,
        whitelist_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), SafeError> {
    info!("state-transition: whitelist_add");

    let StateTransitionRequest {
        whitelist,
        program_id_to_add,
    } = req;

    whitelist
        .push(program_id_to_add)
        .ok_or(SafeErrorCode::WhitelistFull)?;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    safe_authority_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    whitelist_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    whitelist: &'a mut Whitelist,
    program_id_to_add: Pubkey,
}

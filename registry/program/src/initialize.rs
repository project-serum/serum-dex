use serum_common::pack::Pack;
use serum_registry::accounts::{registrar, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    authority: Pubkey,
    withdrawal_timelock: u64,
) -> Result<(), RegistryError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let registrar_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
    })?;

    Registrar::unpack_mut(
        &mut registrar_acc_info.try_borrow_mut_data()?,
        &mut |registrar: &mut Registrar| {
            state_transition(StateTransitionRequest {
                registrar,
                authority,
                withdrawal_timelock,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), RegistryError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), RegistryError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        registrar,
        authority,
        withdrawal_timelock,
    } = req;

    registrar.initialized = true;
    registrar.capabilities_fees_bps = [0; 32];
    registrar.authority = authority;
    registrar.withdrawal_timelock = withdrawal_timelock;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    registrar_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    registrar: &'a mut Registrar,
    authority: Pubkey,
    withdrawal_timelock: u64,
}

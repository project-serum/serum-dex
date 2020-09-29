use serum_common::pack::Pack;
use serum_registry::accounts::Registry;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    capability_id: u8,
    capability_program: Pubkey,
) -> Result<(), RegistryError> {
    info!("handler: register_capability");

    let acc_infos = &mut accounts.iter();

    let registry_authority_acc_info = next_account_info(acc_infos)?;
    let registry_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registry_authority_acc_info,
        registry_acc_info,
        capability_id,
    })?;

    Registry::unpack_mut(
        &mut registry_acc_info.try_borrow_mut_data()?,
        &mut |registry: &mut Registry| {
            state_transition(StateTransitionRequest {
                registry,
                capability_id,
                capability_program,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: register_capability");

    let AccessControlRequest {
        registry_authority_acc_info,
        registry_acc_info,
        capability_id,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: register_capability");

    let StateTransitionRequest {
        mut registry,
        capability_id,
        capability_program,
    } = req;

    registry.capabilities[capability_id as usize] = capability_program;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    registry_authority_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    capability_id: u8,
}

struct StateTransitionRequest<'a> {
    registry: &'a mut Registry,
    capability_id: u8,
    capability_program: Pubkey,
}

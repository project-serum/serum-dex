use crate::access_control;
use serum_common::pack::Pack;
use serum_registry_rewards::accounts::Instance;
use serum_registry_rewards::error::{RewardsError, RewardsErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    nonce: u8,
    registry_program_id: Pubkey,
    dex_program_id: Pubkey,
    authority: Pubkey,
    fee_rate: u64,
) -> Result<(), RewardsError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let instance_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        instance_acc_info,
        vault_acc_info,
        rent_acc_info,
        registrar_acc_info,
        nonce,
        program_id,
        registry_program_id,
        fee_rate,
    })?;

    Instance::unpack_mut(
        &mut instance_acc_info.try_borrow_mut_data()?,
        &mut |instance: &mut Instance| {
            state_transition(StateTransitionRequest {
                instance,
                vault_acc_info,
                nonce,
                registrar_acc_info,
                registry_program_id,
                dex_program_id,
                authority,
                fee_rate,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        instance_acc_info,
        vault_acc_info,
        rent_acc_info,
        registrar_acc_info,
        nonce,
        program_id,
        registry_program_id,
        fee_rate,
    } = req;

    // Authorization: None.

    let rent = access_control::rent(rent_acc_info)?;

    let _ = serum_registry::access_control::registrar(registrar_acc_info, &registry_program_id)
        .map_err(|_| RewardsErrorCode::InvalidRegistrar)?;
    let instance = Instance::unpack(&instance_acc_info.try_borrow_data()?)?;
    if instance.initialized {
        return Err(RewardsErrorCode::AlreadyInitialized)?;
    }
    if instance_acc_info.owner != program_id {
        return Err(RewardsErrorCode::InvalidAccountOwner)?;
    }
    let _ =
        access_control::vault_init(vault_acc_info, instance_acc_info, &rent, nonce, program_id)?;
    if fee_rate <= 0 {
        return Err(RewardsErrorCode::InvalidFeeRate)?;
    }

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        instance,
        vault_acc_info,
        nonce,
        registrar_acc_info,
        registry_program_id,
        dex_program_id,
        authority,
        fee_rate,
    } = req;

    instance.initialized = true;
    instance.vault = *vault_acc_info.key;
    instance.nonce = nonce;
    instance.registrar = *registrar_acc_info.key;
    instance.registry_program_id = registry_program_id;
    instance.dex_program_id = dex_program_id;
    instance.authority = authority;
    instance.fee_rate = fee_rate;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    instance_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    nonce: u8,
    program_id: &'a Pubkey,
    registry_program_id: Pubkey,
    fee_rate: u64,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    instance: &'c mut Instance,
    vault_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    nonce: u8,
    registry_program_id: Pubkey,
    dex_program_id: Pubkey,
    authority: Pubkey,
    fee_rate: u64,
}

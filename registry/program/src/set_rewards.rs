use serum_common::pack::Pack;
use serum_registry::accounts::Registry;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    rewards: Pubkey,
    rewards_return_value: Pubkey,
) -> Result<(), RegistryError> {
    info!("handler: set_rewards");

    let acc_infos = &mut accounts.iter();

    let registry_authority_acc_info = next_account_info(acc_infos)?;
    let registry_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registry_authority_acc_info,
        registry_acc_info,
        rewards,
        rewards_return_value,
    })?;

    Registry::unpack_mut(
        &mut registry_acc_info.try_borrow_mut_data()?,
        &mut |registry: &mut Registry| {
            state_transition(StateTransitionRequest {
                registry,
                rewards,
                rewards_return_value,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), RegistryError> {
    info!("access-control: set_rewards");

    let AccessControlRequest {
        registry_authority_acc_info,
        registry_acc_info,
        rewards,
        rewards_return_value,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), RegistryError> {
    info!("state-transition: set_rewards");

    let StateTransitionRequest {
        registry,
        rewards,
        rewards_return_value,
    } = req;

    registry.rewards = rewards;
    registry.rewards_return_value = rewards_return_value;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    registry_authority_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    rewards: Pubkey,
    rewards_return_value: Pubkey,
}

struct StateTransitionRequest<'a> {
    registry: &'a mut Registry,
    rewards: Pubkey,
    rewards_return_value: Pubkey,
}

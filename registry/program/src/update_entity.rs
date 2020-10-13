use serum_common::pack::Pack;
use serum_registry::accounts::Entity;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    leader: Pubkey,
    capabilities: u32,
) -> Result<(), RegistryError> {
    info!("handler: update_entity");

    let acc_infos = &mut accounts.iter();

    let entity_acc_info = next_account_info(acc_infos)?;
    let entity_leader_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        entity_acc_info,
        entity_leader_acc_info,
    })?;

    Entity::unpack_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            state_transition(StateTransitionRequest {
                entity,
                capabilities,
                leader,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: update_entity");

    let AccessControlRequest {
        entity_acc_info,
        entity_leader_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: update_entity");

    let StateTransitionRequest {
        entity,
        capabilities,
        leader,
    } = req;

    entity.leader = leader;
    entity.capabilities = capabilities;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    entity_acc_info: &'a AccountInfo<'a>,
    entity_leader_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    entity: &'a mut Entity,
    capabilities: u32,
    leader: Pubkey,
}

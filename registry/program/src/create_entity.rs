use serum_common::pack::Pack;
use serum_registry::accounts::{Entity, StakeKind};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    capabilities: u32,
    stake_kind: StakeKind,
) -> Result<(), RegistryError> {
    info!("handler: create_entity");

    let acc_infos = &mut accounts.iter();

    let entity_acc_info = next_account_info(acc_infos)?;
    let entity_leader_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        entity_acc_info,
        entity_leader_acc_info,
        rent_acc_info,
        stake_kind,
    })?;

    Entity::unpack_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            state_transition(StateTransitionRequest {
                leader: entity_leader_acc_info.key,
                entity,
                capabilities,
                stake_kind,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: create_entity");

    let AccessControlRequest {
        entity_acc_info,
        entity_leader_acc_info,
        rent_acc_info,
        stake_kind,
    } = req;

    // TODO: remove in next release.
    if stake_kind != StakeKind::Delegated {
        return Err(RegistryErrorCode::MustBeDelegated)?;
    }

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: create_entity");

    let StateTransitionRequest {
        entity,
        leader,
        capabilities,
        stake_kind,
    } = req;

    entity.initialized = true;
    entity.leader = *leader;
    entity.amount = 0;
    entity.mega_amount = 0;
    entity.capabilities = capabilities;
    entity.stake_kind = stake_kind;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    entity_acc_info: &'a AccountInfo<'a>,
    entity_leader_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    stake_kind: StakeKind,
}

struct StateTransitionRequest<'a> {
    entity: &'a mut Entity,
    leader: &'a Pubkey,
    capabilities: u32,
    stake_kind: StakeKind,
}

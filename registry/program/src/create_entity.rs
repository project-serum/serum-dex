use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, EntityState};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    metadata: Pubkey,
) -> Result<(), RegistryError> {
    msg!("handler: create_entity");

    let acc_infos = &mut accounts.iter();

    let entity_acc_info = next_account_info(acc_infos)?;
    let leader_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        entity_acc_info,
        leader_acc_info,
        registrar_acc_info,
        rent_acc_info,
        program_id,
    })?;

    Entity::unpack_unchecked_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            state_transition(StateTransitionRequest {
                entity,
                metadata,
                leader_acc_info,
                registrar_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    msg!("access-control: create_entity");

    let AccessControlRequest {
        entity_acc_info,
        leader_acc_info,
        registrar_acc_info,
        rent_acc_info,
        program_id,
    } = req;

    // Node leader authorization.
    if !leader_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let _ = access_control::registrar(registrar_acc_info, program_id)?;

    // CreateEntity specific.
    {
        let mut data: &[u8] = &entity_acc_info.try_borrow_data()?;
        let entity = Entity::unpack_unchecked(&mut data)?;
        if entity_acc_info.owner != program_id {
            return Err(RegistryErrorCode::InvalidAccountOwner)?;
        }
        if entity.initialized {
            return Err(RegistryErrorCode::AlreadyInitialized)?;
        }
        if !rent.is_exempt(entity_acc_info.lamports(), entity_acc_info.try_data_len()?) {
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: create_entity");

    let StateTransitionRequest {
        entity,
        leader_acc_info,
        registrar_acc_info,
        metadata,
    } = req;

    entity.initialized = true;
    entity.registrar = *registrar_acc_info.key;
    entity.leader = *leader_acc_info.key;
    entity.balances = Default::default();
    entity.state = EntityState::Inactive;
    entity.metadata = metadata;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    entity_acc_info: &'a AccountInfo<'b>,
    leader_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    leader_acc_info: &'a AccountInfo<'b>,
    entity: &'c mut Entity,
    metadata: Pubkey,
}

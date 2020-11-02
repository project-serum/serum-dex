use crate::pool::{pool_check_get_basket, Pool, PoolConfig};
use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Generation};
use serum_registry::error::RegistryError;
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    info!("handler: update_entity");

    let acc_infos = &mut accounts.iter();

    let generation_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;

    let pool = &Pool::parse_accounts(acc_infos, PoolConfig::GetBasket)?;

    let AccessControlResponse { ref entity } = access_control(AccessControlRequest {
        generation_acc_info,
        entity_acc_info,
        registrar_acc_info,
        program_id,
        pool,
    })?;

    Generation::unpack_mut(
        &mut generation_acc_info.try_borrow_mut_data()?,
        &mut |generation: &mut Generation| {
            state_transition(StateTransitionRequest {
                generation,
                entity,
                entity_acc_info,
                pool,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: update_entity");

    let AccessControlRequest {
        generation_acc_info,
        entity_acc_info,
        registrar_acc_info,
        program_id,
        pool,
    } = req;

    // Authorization: none.

    // Account validation.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    pool_check_get_basket(program_id, pool, registrar_acc_info, &registrar)?;

    let g = Generation::unpack(&generation_acc_info.try_borrow_data()?)?;
    if g.initialized {
        // Only allow the `last_active_price` to change on the Generation.
        access_control::generation_check(
            program_id,
            &g,
            generation_acc_info,
            entity_acc_info,
            entity.generation,
        )?;
    }

    Ok(AccessControlResponse { entity })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: update_entity");

    let StateTransitionRequest {
        generation,
        entity,
        entity_acc_info,
        pool,
    } = req;

    generation.initialized = true;
    generation.entity = *entity_acc_info.key;
    generation.generation = entity.generation;
    generation.last_active_prices = pool.prices().clone();

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    generation_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    pool: &'c Pool<'a, 'b>,
}

struct AccessControlResponse {
    entity: Entity,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    entity_acc_info: &'a AccountInfo<'b>,
    pool: &'c Pool<'a, 'b>,
    entity: &'c Entity,
    generation: &'c mut Generation,
}

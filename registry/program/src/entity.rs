use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::entity::PoolPrices;
use serum_registry::accounts::{Entity, Registrar};
use serum_registry::error::RegistryError;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;

// with_entity should be used for any instruction relying on the most up to
// date `state` of an Entity.
//
//
// As time, passes, it's possible an Entity's internal FSM *should* have
// transitioned (i.e., from PendingDeactivation -> Inactive), but didn't
// because no transaction was invoked.
//
// This method transitions the Entity's state, before, performing the action
// provided by the given closure, and after.
#[inline(always)]
pub fn with_entity<F>(req: EntityContext, f: &mut F) -> Result<(), RegistryError>
where
    F: FnMut(&mut Entity, &Registrar, &Clock) -> Result<(), RegistryError>,
{
    let EntityContext {
        entity_acc_info,
        registrar_acc_info,
        clock_acc_info,
        program_id,
        prices,
    } = req;
    Entity::unpack_unchecked_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            let clock = access_control::clock(&clock_acc_info)?;
            let registrar = access_control::registrar(&registrar_acc_info, program_id)?;
            let _ = access_control::entity_check(
                entity,
                entity_acc_info,
                registrar_acc_info,
                program_id,
            )?;

            entity.transition_activation_if_needed(&prices, &registrar, &clock);
            let r = f(entity, &registrar, &clock)?;
            entity.transition_activation_if_needed(&prices, &registrar, &clock);

            Ok(r)
        },
    )?;
    Ok(())
}

pub struct EntityContext<'a, 'b> {
    pub entity_acc_info: &'a AccountInfo<'b>,
    pub registrar_acc_info: &'a AccountInfo<'b>,
    pub clock_acc_info: &'a AccountInfo<'b>,
    pub program_id: &'a Pubkey,
    // Should use the redemption (not creation) price since it's conservative
    // by rounding down instead of up.
    //
    // For the `stake` instruction, in practice, we use the creation price,
    // since we hit the instruction limit, otherwise (lots of CPI between
    // registry -> pool -> retbuf).
    pub prices: &'a PoolPrices,
}

use crate::entity::{with_entity, EntityContext};
use crate::pool::{pool_check_create, Pool, PoolConfig};
use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    spt_amount: u64,
) -> Result<(), RegistryError> {
    info!("handler: stake");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    let ref pool = Pool::parse_accounts(
        acc_infos,
        PoolConfig::Execute {
            registrar_acc_info,
            token_program_acc_info,
            is_create: true,
        },
    )?;

    let ctx = EntityContext {
        entity_acc_info,
        registrar_acc_info,
        clock_acc_info,
        program_id,
        prices: pool.prices(),
    };
    with_entity(ctx, &mut |entity: &mut Entity,
                           registrar: &Registrar,
                           ref clock: &Clock| {
        access_control(AccessControlRequest {
            member_acc_info,
            registrar_acc_info,
            beneficiary_acc_info,
            entity_acc_info,
            spt_amount,
            entity,
            program_id,
            registrar,
            pool,
        })?;
        Member::unpack_mut(
            &mut member_acc_info.try_borrow_mut_data()?,
            &mut |member: &mut Member| {
                state_transition(StateTransitionRequest {
                    entity,
                    member,
                    spt_amount,
                    pool,
                    clock,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: stake");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        registrar_acc_info,
        registrar,
        spt_amount,
        entity,
        program_id,
        pool,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    access_control::entity_check(entity, entity_acc_info, registrar_acc_info, program_id)?;
    let member = access_control::member_join(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let _ = pool_check_create(program_id, pool, registrar_acc_info, registrar, &member)?;

    // Stake specific.
    {
        // Can the member afford the staking tokens?
        if !member.can_afford(pool.prices(), spt_amount, pool.is_mega())? {
            return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
        }
        // All stake from a previous generation must be withdrawn before adding
        // stake for a new generation.
        if member.generation != entity.generation {
            if !member.stake_is_empty() {
                return Err(RegistryErrorCode::StaleStakeNeedsWithdrawal)?;
            }
        }
        // Only activated nodes can stake.
        if !entity.meets_activation_requirements(pool.prices(), &registrar) {
            return Err(RegistryErrorCode::EntityNotActivated)?;
        }

        // Will this new stake put the entity over the maximum allowable limit?
        let spt_worth = pool.prices().srm_equivalent(spt_amount, pool.is_mega());
        if spt_worth + entity.amount_equivalent(pool.prices()) > registrar.max_stake_per_entity {
            return Err(RegistryErrorCode::EntityMaxStake)?;
        }
    }

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        member,
        spt_amount,
        pool,
        clock,
    } = req;

    // Transfer funds into the staking pool, minting to the staking token.
    pool.create(spt_amount)?;

    // Update accounts for bookeeping.
    member.generation = entity.generation;
    member.last_stake_ts = clock.unix_timestamp;
    member.spt_did_create(pool.prices(), spt_amount, pool.is_mega())?;
    entity.spt_did_create(pool.prices(), spt_amount, pool.is_mega())?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
    pool: &'c Pool<'a, 'b>,
    entity: &'c Entity,
    spt_amount: u64,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    pool: &'c Pool<'a, 'b>,
    entity: &'c mut Entity,
    member: &'c mut Member,
    spt_amount: u64,
    clock: &'c Clock,
}

use crate::common::invoke_token_transfer;
use crate::entity::{with_entity, EntityContext};
use crate::pool::{pool_check, Pool, PoolConfig};
use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{vault, Entity, Member, Registrar};
use serum_registry::error::RegistryError;
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use std::convert::Into;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    spt_amount: u64,
) -> Result<(), RegistryError> {
    info!("handler: slash");

    let acc_infos = &mut accounts.iter();

    let registrar_authority_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    let ref pool = Pool::parse_accounts(
        acc_infos,
        PoolConfig::Execute {
            registrar_acc_info,
            token_program_acc_info,
            is_create: false,
        },
    )?;

    let vault_acc_info = pool
        .registry_vault_acc_infos
        .as_ref()
        .expect("transact config")[0];
    let mega_vault_acc_info = match pool.is_mega() {
        false => None,
        true => Some(
            pool.registry_vault_acc_infos
                .as_ref()
                .expect("transact config")[1],
        ),
    };

    let ctx = EntityContext {
        entity_acc_info,
        registrar_acc_info,
        clock_acc_info,
        program_id,
        prices: pool.prices(),
    };
    with_entity(ctx, &mut |entity: &mut Entity,
                           registrar: &Registrar,
                           _: &Clock| {
        access_control(AccessControlRequest {
            registrar,
            registrar_authority_acc_info,
            registrar_acc_info,
            member_acc_info,
            entity_acc_info,
            vault_acc_info,
            vault_authority_acc_info,
            mega_vault_acc_info,
            program_id,
            pool,
        })?;
        Member::unpack_mut(
            &mut member_acc_info.try_borrow_mut_data()?,
            &mut |member: &mut Member| {
                state_transition(StateTransitionRequest {
                    vault_acc_info,
                    mega_vault_acc_info,
                    vault_authority_acc_info,
                    registrar_acc_info,
                    token_program_acc_info,
                    registrar,
                    entity,
                    member,
                    pool,
                    spt_amount,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })?;

    Ok(())
}

#[inline(always)]
fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: slash");

    let AccessControlRequest {
        registrar,
        registrar_acc_info,
        registrar_authority_acc_info,
        member_acc_info,
        entity_acc_info,
        vault_acc_info,
        mega_vault_acc_info,
        vault_authority_acc_info,
        program_id,
        pool,
    } = req;

    // Authorization.
    let _ =
        access_control::governance(program_id, registrar_acc_info, registrar_authority_acc_info)?;

    // Account validation.
    let _entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let member = access_control::member_raw(member_acc_info, entity_acc_info, program_id)?;
    let _vault = access_control::vault_join(
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        registrar,
        program_id,
    )?;
    if let Some(mega_vault_acc_info) = mega_vault_acc_info {
        let _mega_vault = access_control::vault_join(
            mega_vault_acc_info,
            vault_authority_acc_info,
            registrar_acc_info,
            registrar,
            program_id,
        )?;
    }
    pool_check(
        program_id,
        pool,
        registrar_authority_acc_info,
        &registrar,
        &member,
    )?;

    // Slash specific: none.

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: slash");

    let StateTransitionRequest {
        spt_amount,
        entity,
        member,
        pool,
        vault_acc_info,
        mega_vault_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        registrar,
        registrar_acc_info,
    } = req;

    // Burn the staking pool tokens (redeem but don't credit the Member).
    pool.redeem(spt_amount)?;
    let confiscated_basket = pool
        .prices()
        .basket_quantities(spt_amount, pool.is_mega())?;

    // Transfer the funds back to the pool (rewarding all other holders).
    invoke_token_transfer(
        vault_acc_info,
        pool.pool_asset_vault_acc_infos[0],
        vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
        confiscated_basket[0],
    )?;
    if pool.pool_asset_vault_acc_infos.len() == 2 {
        invoke_token_transfer(
            mega_vault_acc_info.expect("mega specified"),
            pool.pool_asset_vault_acc_infos[1],
            vault_authority_acc_info,
            token_program_acc_info,
            &[&vault::signer_seeds(
                registrar_acc_info.key,
                &registrar.nonce,
            )],
            confiscated_basket[1],
        )?;
    }

    // Book keeping.
    entity.slash(spt_amount, pool.is_mega());
    member.slash(spt_amount, pool.is_mega());

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar_authority_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: Option<&'a AccountInfo<'b>>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    pool: &'c Pool<'a, 'b>,
    registrar: &'c Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: Option<&'a AccountInfo<'b>>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    entity: &'c mut Entity,
    member: &'c mut Member,
    pool: &'c Pool<'a, 'b>,
    spt_amount: u64,
}

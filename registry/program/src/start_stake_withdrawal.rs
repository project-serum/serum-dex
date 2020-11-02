use crate::common::invoke_token_transfer;
use crate::entity::{with_entity, EntityContext};
use crate::pool::{pool_check, Pool, PoolConfig};
use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::entity::{EntityState, PoolPrices};
use serum_registry::accounts::pending_withdrawal::PendingPayment;
use serum_registry::accounts::vault;
use serum_registry::accounts::{Entity, Generation, Member, PendingWithdrawal, Registrar};
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
    info!("handler: start_stake_withdrawal");

    let acc_infos = &mut accounts.iter();

    let pending_withdrawal_acc_info = next_account_info(acc_infos)?;

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;

    let tok_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    let ref pool = Pool::parse_accounts(
        acc_infos,
        PoolConfig::Execute {
            registrar_acc_info,
            token_program_acc_info: tok_program_acc_info,
            is_create: false,
        },
    )?;

    // Prior initialization of the Generation account is optional.
    let generation_acc_info = acc_infos.next();

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
                           clock: &Clock| {
        let AccessControlResponse { ref generation } = access_control(AccessControlRequest {
            pending_withdrawal_acc_info,
            beneficiary_acc_info,
            registrar_acc_info,
            member_acc_info,
            entity_acc_info,
            rent_acc_info,
            program_id,
            vault_acc_info,
            mega_vault_acc_info,
            vault_authority_acc_info,
            generation_acc_info,
            registrar,
            pool,
        })?;
        PendingWithdrawal::unpack_mut(
            &mut pending_withdrawal_acc_info.try_borrow_mut_data()?,
            &mut |pending_withdrawal: &mut PendingWithdrawal| {
                Member::unpack_mut(
                    &mut member_acc_info.try_borrow_mut_data()?,
                    &mut |member: &mut Member| {
                        state_transition(StateTransitionRequest {
                            pending_withdrawal,
                            registrar,
                            member,
                            entity,
                            member_acc_info,
                            clock,
                            spt_amount,
                            pool,
                            vault_acc_info,
                            mega_vault_acc_info,
                            vault_authority_acc_info,
                            tok_program_acc_info,
                            registrar_acc_info,
                            generation,
                        })
                        .map_err(Into::into)
                    },
                )
            },
        )
        .map_err(Into::into)
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: start_stake_withdrawal");

    let AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
        program_id,
        vault_acc_info,
        mega_vault_acc_info,
        vault_authority_acc_info,
        registrar,
        pool,
        generation_acc_info,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let member = access_control::member_join(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let generation = generation_acc_info
        .map(|generation_acc_info| {
            access_control::generation(generation_acc_info, entity_acc_info, &member, program_id)
        })
        // Swap the option and result positions.
        .map_or(Ok(None), |res| res.map(Some))?;

    pool_check(program_id, pool, registrar_acc_info, registrar, &member)?;
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

    // StartStakeWithdrawal specific.
    {
        let pw = PendingWithdrawal::unpack(&pending_withdrawal_acc_info.try_borrow_data()?)?;
        if pending_withdrawal_acc_info.owner != program_id {
            return Err(RegistryErrorCode::InvalidAccountOwner)?;
        }
        if pw.initialized {
            return Err(RegistryErrorCode::AlreadyInitialized)?;
        }
        if !rent.is_exempt(
            pending_withdrawal_acc_info.lamports(),
            pending_withdrawal_acc_info.try_data_len()?,
        ) {
            // TODO: this doesn't actually need to be rent exempt, since the account
            //       only needs to live during the pending withdrawal window.
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    Ok(AccessControlResponse { generation })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: start_stake_withdrawal");

    let StateTransitionRequest {
        pending_withdrawal,
        registrar,
        entity,
        member,
        member_acc_info,
        clock,
        spt_amount,
        pool,
        vault_acc_info,
        mega_vault_acc_info,
        vault_authority_acc_info,
        tok_program_acc_info,
        registrar_acc_info,
        generation,
    } = req;

    // Redeem the `spt_amount` tokens for the underlying basket, transferring
    // the assets into this program's vaults.
    pool.redeem(spt_amount)?;

    // The amounts that were transferred from `pool.redeem`.
    let mut asset_amounts = pool
        .prices()
        .basket_quantities(spt_amount, pool.is_mega())?;

    // Inactive entities don't receive rewards while inactive, so return the
    // excess amounts back into the pool.
    if entity.state == EntityState::Inactive {
        // TODO: consider alternatives to returning funds back into pool, e.g.,
        //       a separate vault/community-fund.
        asset_amounts = pool_return_forfeited_assets(
            pool,
            match generation.as_ref() {
                None => &member.last_active_prices,
                Some(g) => &g.last_active_prices,
            },
            asset_amounts,
            vault_acc_info,
            mega_vault_acc_info,
            vault_authority_acc_info,
            tok_program_acc_info,
            registrar_acc_info,
            registrar,
            spt_amount,
        )?;
    }

    // Bookeeping.
    member.spt_did_redeem_start(spt_amount, pool.is_mega());
    entity.spt_did_redeem_start(spt_amount, pool.is_mega());

    // Print the pending withdrawal receipt.
    {
        pending_withdrawal.initialized = true;
        pending_withdrawal.burned = false;
        pending_withdrawal.member = *member_acc_info.key;
        pending_withdrawal.start_ts = clock.unix_timestamp;
        pending_withdrawal.end_ts = clock.unix_timestamp + registrar.deactivation_timelock();
        pending_withdrawal.spt_amount = spt_amount;
        pending_withdrawal.pool = *pool.pool_acc_info.key;
        pending_withdrawal.payment = PendingPayment {
            asset_amount: asset_amounts[0],
            mega_asset_amount: match pool.is_mega() {
                true => asset_amounts[1],
                false => 0,
            },
        };
    }

    Ok(())
}

// Returns the basket amount the staker should get when withdrawing from an
// inactive node entity.
//
// If the node is inactive, mark the price of the staking pool token
// to the price at the last time this member staked. Transfer any excess
// tokens back into the pool (i.e., when marking to the current price).
fn pool_return_forfeited_assets<'a, 'b, 'c>(
    pool: &'c Pool<'a, 'b>,
    prices: &'c PoolPrices,
    current_asset_amounts: Vec<u64>,
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: Option<&'a AccountInfo<'b>>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    tok_program_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    spt_amount: u64,
) -> Result<Vec<u64>, RegistryError> {
    // The basket amounts the user will receive upon withdrawal.
    let marked_asset_amounts = prices.basket_quantities(spt_amount, pool.is_mega())?;
    assert!(current_asset_amounts.len() == marked_asset_amounts.len());
    assert!(current_asset_amounts.len() == 2);

    // The basket amounts to return to the pool.
    let excess_asset_amounts: Vec<u64> = current_asset_amounts
        .iter()
        .zip(marked_asset_amounts.iter())
        .map(|(current, marked)| current - marked)
        .collect();
    assert!(pool.pool_asset_vault_acc_infos.len() == 2);

    // Transfer the excess SRM and MSRM back to the pool.
    invoke_token_transfer(
        vault_acc_info,
        pool.pool_asset_vault_acc_infos[0], // SRM.
        vault_authority_acc_info,
        tok_program_acc_info,
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
        excess_asset_amounts[0],
    )?;
    if pool.pool_asset_vault_acc_infos.len() == 2 {
        invoke_token_transfer(
            mega_vault_acc_info.expect("mega specified"),
            pool.pool_asset_vault_acc_infos[1], // MSRM.
            vault_authority_acc_info,
            tok_program_acc_info,
            &[&vault::signer_seeds(
                registrar_acc_info.key,
                &registrar.nonce,
            )],
            excess_asset_amounts[1],
        )?;
    }
    Ok(marked_asset_amounts)
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: Option<&'a AccountInfo<'b>>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    generation_acc_info: Option<&'a AccountInfo<'b>>,
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
    pool: &'c Pool<'a, 'b>,
}

struct AccessControlResponse {
    generation: Option<Generation>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: Option<&'a AccountInfo<'b>>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    tok_program_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal: &'c mut PendingWithdrawal,
    pool: &'c Pool<'a, 'b>,
    entity: &'c mut Entity,
    member: &'c mut Member,
    registrar: &'c Registrar,
    clock: &'c Clock,
    generation: &'c Option<Generation>,
    spt_amount: u64,
}

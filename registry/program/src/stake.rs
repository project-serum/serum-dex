use crate::common::entity::{with_entity, EntityContext};
use crate::switch_entity::AssetAccInfos;
use serum_common::pack::Pack;
use serum_common::program::{invoke_mint_tokens, invoke_token_transfer};
use serum_registry::access_control::{self, StakeAssets};
use serum_registry::accounts::{vault, Entity, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    spt_amount: u64,
    ref balance_id: Pubkey,
) -> Result<(), RegistryError> {
    msg!("handler: stake");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let member_vault_acc_info = next_account_info(acc_infos)?;
    let member_vault_authority_acc_info = next_account_info(acc_infos)?;
    let member_vault_stake_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let spt_acc_info = next_account_info(acc_infos)?;
    let reward_q_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let mut asset_acc_infos = vec![];
    while acc_infos.len() > 0 {
        asset_acc_infos.push(AssetAccInfos {
            owner_acc_info: next_account_info(acc_infos)?,
            spt_acc_info: next_account_info(acc_infos)?,
            spt_mega_acc_info: next_account_info(acc_infos)?,
        });
    }
    let asset_acc_infos = &asset_acc_infos;

    let ctx = EntityContext {
        entity_acc_info,
        registrar_acc_info,
        clock_acc_info,
        program_id,
    };
    with_entity(ctx, &mut |entity: &mut Entity,
                           registrar: &Registrar,
                           ref clock: &Clock| {
        let AccessControlResponse { is_mega } = access_control(AccessControlRequest {
            member_acc_info,
            beneficiary_acc_info,
            entity_acc_info,
            spt_amount,
            entity,
            program_id,
            registrar,
            registrar_acc_info,
            member_vault_acc_info,
            member_vault_authority_acc_info,
            member_vault_stake_acc_info,
            pool_mint_acc_info,
            reward_q_acc_info,
            spt_acc_info,
            asset_acc_infos,
            balance_id,
        })?;
        Member::unpack_mut(
            &mut member_acc_info.try_borrow_mut_data()?,
            &mut |member: &mut Member| {
                state_transition(StateTransitionRequest {
                    entity,
                    member,
                    spt_amount,
                    clock,
                    is_mega,
                    registrar,
                    registrar_acc_info,
                    member_vault_acc_info,
                    member_vault_authority_acc_info,
                    member_vault_stake_acc_info,
                    spt_acc_info,
                    token_program_acc_info,
                    pool_mint_acc_info,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: stake");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        registrar_acc_info,
        registrar,
        spt_amount,
        entity,
        program_id,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        member_vault_stake_acc_info,
        spt_acc_info,
        pool_mint_acc_info,
        reward_q_acc_info,
        asset_acc_infos,
        balance_id,
    } = req;

    assert!(spt_amount > 0);

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let member = access_control::member_entity(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let (_member_vault, is_mega) = access_control::member_vault(
        &member,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        registrar,
        program_id,
        balance_id,
    )?;
    let (_member_vault_stake, is_mega_stake) = access_control::member_vault_stake(
        &member,
        member_vault_stake_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
        balance_id,
    )?;
    assert!(is_mega == is_mega_stake);
    let _pool_token = access_control::member_pool_token(
        &member,
        spt_acc_info,
        pool_mint_acc_info,
        balance_id,
        is_mega,
    )?;
    let _pool_mint = access_control::pool_mint(pool_mint_acc_info, &registrar, is_mega)?;
    let _reward_q = access_control::reward_event_q(
        reward_q_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;
    let assets = {
        // Ensure the given asset ids are unique.
        let mut balance_ids: Vec<Pubkey> = asset_acc_infos
            .iter()
            .map(|a| *a.owner_acc_info.key)
            .collect();
        balance_ids.sort();
        balance_ids.dedup();
        if balance_ids.len() != member.balances.len() {
            return Err(RegistryErrorCode::InvalidAssetsLen)?;
        }
        // Validate each asset.
        let mut assets = vec![];
        for a in asset_acc_infos.iter() {
            let (spt, is_mega) = access_control::member_spt(
                &member,
                a.spt_acc_info,
                member_vault_authority_acc_info,
                registrar_acc_info,
                &registrar,
                program_id,
                a.owner_acc_info.key,
            )?;
            assert!(!is_mega);
            let (spt_mega, is_mega) = access_control::member_spt(
                &member,
                a.spt_mega_acc_info,
                member_vault_authority_acc_info,
                registrar_acc_info,
                &registrar,
                program_id,
                a.owner_acc_info.key,
            )?;
            assert!(is_mega);
            assets.push(StakeAssets { spt, spt_mega });
        }
        assets
    };
    // Does the Member account have any unprocessed rewards?
    if access_control::reward_cursor_needs_update(reward_q_acc_info, &member, &assets, &registrar)?
    {
        return Err(RegistryErrorCode::RewardCursorNeedsUpdate)?;
    }
    // Can only stake to active entities.
    if !entity.meets_activation_requirements() {
        // Staking MSRM will activate, so allow it.
        if !is_mega {
            return Err(RegistryErrorCode::EntityNotActivated)?;
        }
    }
    // Will this new stake put the entity over the maximum allowable limit?
    if entity.stake_will_max(spt_amount, is_mega, &registrar) {
        return Err(RegistryErrorCode::EntityMaxStake)?;
    }

    Ok(AccessControlResponse { is_mega })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        member,
        spt_amount,
        is_mega,
        registrar_acc_info,
        registrar,
        member_vault_acc_info,
        member_vault_authority_acc_info,
        member_vault_stake_acc_info,
        spt_acc_info,
        token_program_acc_info,
        pool_mint_acc_info,
        clock,
    } = req;

    let signer_seeds = vault::signer_seeds(registrar_acc_info.key, &registrar.nonce);

    // Mint pool tokens to member.
    invoke_mint_tokens(
        pool_mint_acc_info,
        spt_acc_info,
        member_vault_authority_acc_info,
        token_program_acc_info,
        &[&signer_seeds],
        spt_amount,
    )?;

    // Convert from stake token units to srm/msrm units.
    let token_amount = {
        let rate = match is_mega {
            false => registrar.stake_rate,
            true => registrar.stake_rate_mega,
        };
        spt_amount.checked_mul(rate).unwrap()
    };

    // Transfer from deposit vault to stake vault.
    invoke_token_transfer(
        member_vault_acc_info,
        member_vault_stake_acc_info,
        member_vault_authority_acc_info,
        token_program_acc_info,
        &[&signer_seeds],
        token_amount,
    )?;

    // Bookeeping.
    member.last_stake_ts = clock.unix_timestamp;
    entity.spt_did_stake(spt_amount, is_mega)?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    member_vault_stake_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    reward_q_acc_info: &'a AccountInfo<'b>,
    asset_acc_infos: &'c [AssetAccInfos<'a, 'b>],
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
    entity: &'c Entity,
    spt_amount: u64,
    balance_id: &'c Pubkey,
}

struct AccessControlResponse {
    is_mega: bool,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    entity: &'c mut Entity,
    member: &'c mut Member,
    clock: &'c Clock,
    spt_amount: u64,
    is_mega: bool,
    registrar: &'c Registrar,
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    member_vault_stake_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
}

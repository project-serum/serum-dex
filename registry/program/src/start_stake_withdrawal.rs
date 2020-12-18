use crate::common::entity::{with_entity, EntityContext};
use crate::switch_entity::AssetAccInfos;
use serum_common::pack::Pack;
use serum_common::program::{invoke_burn_tokens, invoke_token_transfer};
use serum_registry::access_control::{self, StakeAssets};
use serum_registry::accounts::vault;
use serum_registry::accounts::{Entity, PendingWithdrawal, Registrar};
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
    msg!("handler: start_stake_withdrawal");

    let acc_infos = &mut accounts.iter();

    let pending_withdrawal_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let member_vault_pw_acc_info = next_account_info(acc_infos)?;
    let member_vault_authority_acc_info = next_account_info(acc_infos)?;
    let member_vault_stake_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let spt_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let tok_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let reward_q_acc_info = next_account_info(acc_infos)?;
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
                           clock: &Clock| {
        let AccessControlResponse { is_mega } = access_control(AccessControlRequest {
            pending_withdrawal_acc_info,
            beneficiary_acc_info,
            registrar_acc_info,
            member_acc_info,
            entity_acc_info,
            rent_acc_info,
            program_id,
            member_vault_pw_acc_info,
            member_vault_authority_acc_info,
            member_vault_stake_acc_info,
            pool_mint_acc_info,
            spt_acc_info,
            reward_q_acc_info,
            asset_acc_infos,
            registrar,
            balance_id,
        })?;
        PendingWithdrawal::unpack_mut(
            &mut pending_withdrawal_acc_info.try_borrow_mut_data()?,
            &mut |pending_withdrawal: &mut PendingWithdrawal| {
                state_transition(StateTransitionRequest {
                    pending_withdrawal,
                    registrar,
                    entity,
                    member_acc_info,
                    clock,
                    spt_amount,
                    tok_program_acc_info,
                    registrar_acc_info,
                    member_vault_pw_acc_info,
                    member_vault_authority_acc_info,
                    member_vault_stake_acc_info,
                    pool_mint_acc_info,
                    spt_acc_info,
                    is_mega,
                    balance_id,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: start_stake_withdrawal");

    let AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
        program_id,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
        member_vault_stake_acc_info,
        pool_mint_acc_info,
        spt_acc_info,
        reward_q_acc_info,
        asset_acc_infos,
        registrar,
        balance_id,
    } = req;

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
    let (_member_vault, is_mega) = access_control::member_vault_pending_withdrawal(
        &member,
        member_vault_pw_acc_info,
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
        registrar,
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
    let rent = access_control::rent(rent_acc_info)?;

    // Pending withdrawal account.
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
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    let _reward_q = access_control::reward_event_q(
        reward_q_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;
    let assets = {
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

    Ok(AccessControlResponse { is_mega })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: start_stake_withdrawal");

    let StateTransitionRequest {
        pending_withdrawal,
        registrar,
        entity,
        member_acc_info,
        clock,
        spt_amount,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
        member_vault_stake_acc_info,
        pool_mint_acc_info,
        spt_acc_info,
        tok_program_acc_info,
        registrar_acc_info,
        is_mega,
        balance_id,
    } = req;

    let signer_seeds = vault::signer_seeds(registrar_acc_info.key, &registrar.nonce);

    // Burn pool tokens.
    invoke_burn_tokens(
        spt_acc_info,
        pool_mint_acc_info,
        member_vault_authority_acc_info,
        tok_program_acc_info,
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

    // Transfer from stake vault to pending vault.
    invoke_token_transfer(
        member_vault_stake_acc_info,
        member_vault_pw_acc_info,
        member_vault_authority_acc_info,
        tok_program_acc_info,
        &[&signer_seeds],
        token_amount,
    )?;

    // Bookeeping.
    entity.spt_did_unstake(spt_amount, is_mega);

    // Print pending withdrawal receipt.
    pending_withdrawal.initialized = true;
    pending_withdrawal.burned = false;
    pending_withdrawal.member = *member_acc_info.key;
    pending_withdrawal.start_ts = clock.unix_timestamp;
    pending_withdrawal.end_ts = clock.unix_timestamp + registrar.withdrawal_timelock;
    pending_withdrawal.amount = token_amount;
    pending_withdrawal.pool = *pool_mint_acc_info.key;
    pending_withdrawal.balance_id = *balance_id;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    member_vault_pw_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    member_vault_stake_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    reward_q_acc_info: &'a AccountInfo<'b>,
    asset_acc_infos: &'c [AssetAccInfos<'a, 'b>],
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
    balance_id: &'c Pubkey,
}

struct AccessControlResponse {
    is_mega: bool,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    member_vault_pw_acc_info: &'a AccountInfo<'b>,
    member_vault_authority_acc_info: &'a AccountInfo<'b>,
    tok_program_acc_info: &'a AccountInfo<'b>,
    member_vault_stake_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal: &'c mut PendingWithdrawal,
    entity: &'c mut Entity,
    registrar: &'c Registrar,
    clock: &'c Clock,
    spt_amount: u64,
    is_mega: bool,
    balance_id: &'c Pubkey,
}

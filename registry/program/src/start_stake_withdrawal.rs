use crate::common::entity::{with_entity, EntityContext};
use serum_common::pack::Pack;
use serum_common::program::{invoke_burn_tokens, invoke_token_transfer};
use serum_registry::access_control;
use serum_registry::accounts::vault;
use serum_registry::accounts::{Entity, Member, PendingWithdrawal, Registrar};
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
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let pool_vault_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let spt_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let tok_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

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
            vault_acc_info,
            vault_authority_acc_info,
            pool_vault_acc_info,
            pool_mint_acc_info,
            spt_acc_info,
            registrar,
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
                            tok_program_acc_info,
                            registrar_acc_info,
                            vault_acc_info,
                            vault_authority_acc_info,
                            pool_vault_acc_info,
                            pool_mint_acc_info,
                            spt_acc_info,
                            is_mega,
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
        vault_authority_acc_info,
        pool_vault_acc_info,
        pool_mint_acc_info,
        spt_acc_info,
        registrar,
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
    let _vault = access_control::vault_authenticated(
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        registrar,
        program_id,
    )?;
    let (_pool_vault, is_mega) = access_control::pool_vault(pool_vault_acc_info, &registrar)?;
    let _pool_mint = access_control::pool_mint(pool_mint_acc_info, &registrar, is_mega)?;
    let _pool_token =
        access_control::pool_token(spt_acc_info, pool_mint_acc_info, &member, is_mega)?;

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
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    Ok(AccessControlResponse { is_mega })
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
        vault_acc_info,
        vault_authority_acc_info,
        pool_vault_acc_info,
        pool_mint_acc_info,
        spt_acc_info,
        tok_program_acc_info,
        registrar_acc_info,
        is_mega,
    } = req;

    let signer_seeds = vault::signer_seeds(registrar_acc_info.key, &registrar.nonce);

    // Burn pool tokens.
    invoke_burn_tokens(
        spt_acc_info,
        pool_mint_acc_info,
        vault_authority_acc_info,
        tok_program_acc_info,
        &[&signer_seeds],
        spt_amount,
    )?;

    // Transfer from pool vault to deposit vault.
    invoke_token_transfer(
        pool_vault_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        tok_program_acc_info,
        &[&signer_seeds],
        spt_amount,
    )?;

    // Bookeeping.
    member.spt_did_unstake_start(spt_amount, is_mega);
    entity.spt_did_unstake_start(spt_amount, is_mega);

    // Print pending withdrawal receipt.
    pending_withdrawal.initialized = true;
    pending_withdrawal.burned = false;
    pending_withdrawal.member = *member_acc_info.key;
    pending_withdrawal.start_ts = clock.unix_timestamp;
    pending_withdrawal.end_ts = clock.unix_timestamp + registrar.deactivation_timelock;
    pending_withdrawal.spt_amount = spt_amount;
    pending_withdrawal.pool = *pool_vault_acc_info.key;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
}

struct AccessControlResponse {
    is_mega: bool,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    tok_program_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    pending_withdrawal: &'c mut PendingWithdrawal,
    entity: &'c mut Entity,
    member: &'c mut Member,
    registrar: &'c Registrar,
    clock: &'c Clock,
    spt_amount: u64,
    is_mega: bool,
}

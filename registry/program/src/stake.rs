use crate::common::entity::{with_entity, EntityContext};
use serum_common::pack::Pack;
use serum_common::program::{invoke_mint_tokens, invoke_token_transfer};
use serum_registry::access_control;
use serum_registry::accounts::{vault, Entity, Member, Registrar};
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
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let pool_vault_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let spt_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

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
            vault_acc_info,
            vault_authority_acc_info,
            pool_vault_acc_info,
            pool_mint_acc_info,
            spt_acc_info,
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
                    vault_acc_info,
                    vault_authority_acc_info,
                    pool_vault_acc_info,
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
        vault_acc_info,
        vault_authority_acc_info,
        pool_vault_acc_info,
        spt_acc_info,
        pool_mint_acc_info,
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

    // Stake specific.
    {
        if !member.can_afford(spt_amount, is_mega) {
            return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
        }
        if !entity.meets_activation_requirements(&registrar) {
            return Err(RegistryErrorCode::EntityNotActivated)?;
        }

        // Will this new stake put the entity over the maximum allowable limit?
        let spt_worth = {
            if is_mega {
                spt_amount * 1_000_000
            } else {
                spt_amount
            }
        };
        if spt_worth + entity.amount_equivalent() > registrar.max_stake_per_entity {
            return Err(RegistryErrorCode::EntityMaxStake)?;
        }
    }

    Ok(AccessControlResponse { is_mega })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        member,
        spt_amount,
        is_mega,
        registrar_acc_info,
        registrar,
        vault_acc_info,
        vault_authority_acc_info,
        pool_vault_acc_info,
        spt_acc_info,
        token_program_acc_info,
        pool_mint_acc_info,
        clock,
    } = req;

    let signer_seeds = vault::signer_seeds(registrar_acc_info.key, &registrar.nonce);

    // Mint pool tokens.
    invoke_mint_tokens(
        pool_mint_acc_info,
        spt_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[&signer_seeds],
        spt_amount,
    )?;

    // Transfer from the deposit vault to pool vault.
    invoke_token_transfer(
        vault_acc_info,
        pool_vault_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[&signer_seeds],
        spt_amount,
    )?;

    // Bookeeping.
    member.last_stake_ts = clock.unix_timestamp;
    member.spt_did_stake(spt_amount, is_mega)?;
    entity.spt_did_stake(spt_amount, is_mega)?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    registrar: &'c Registrar,
    entity: &'c Entity,
    spt_amount: u64,
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
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
}

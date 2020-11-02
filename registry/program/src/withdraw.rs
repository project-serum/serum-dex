use crate::common::invoke_token_transfer;
use crate::entity::{with_entity, EntityContext};
use crate::pool::{pool_check, Pool, PoolConfig};
use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{vault, Entity, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use spl_token::state::Account as TokenAccount;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> Result<(), RegistryError> {
    info!("handler: withdraw");

    let acc_infos = &mut accounts.iter();

    // Lockup whitelist relay interface.
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;

    // Program specfic.
    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;

    let pool = &Pool::parse_accounts(acc_infos, PoolConfig::GetBasket)?;

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
        let AccessControlResponse { ref depositor } = access_control(AccessControlRequest {
            vault_authority_acc_info,
            depositor_acc_info,
            member_acc_info,
            beneficiary_acc_info,
            entity_acc_info,
            vault_acc_info,
            program_id,
            registrar_acc_info,
            registrar,
            depositor_authority_acc_info,
            amount,
            pool,
        })?;
        Member::unpack_mut(
            &mut member_acc_info.try_borrow_mut_data()?,
            &mut |member: &mut Member| {
                state_transition(StateTransitionRequest {
                    entity,
                    member,
                    amount,
                    registrar: &registrar,
                    registrar_acc_info,
                    vault_acc_info,
                    vault_authority_acc_info,
                    depositor_acc_info,
                    token_program_acc_info,
                    depositor,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: withdraw");

    let AccessControlRequest {
        vault_authority_acc_info,
        depositor_acc_info,
        member_acc_info,
        beneficiary_acc_info,
        entity_acc_info,
        vault_acc_info,
        registrar_acc_info,
        registrar,
        depositor_authority_acc_info,
        program_id,
        amount,
        pool,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }
    if !depositor_authority_acc_info.is_signer {
        // This check is not strictly necessary. It is used to prevent people
        // from shooting themselves in the foot, i.e., withdrawing to a delegate
        // program without the delegate program signing.
        //
        // In the common case (i.e. withdrawing without a delegate program),
        // this authority will be the same as the beneficiary.
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let member = access_control::member_join(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        program_id,
    )?;
    let _vault = access_control::vault_join(
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;
    let depositor = access_control::token(depositor_acc_info, depositor_authority_acc_info.key)?;
    pool_check(program_id, pool, registrar_acc_info, &registrar, &member)?;

    // Withdraw specific.
    let is_mega = registrar.is_mega(*vault_acc_info.key)?;
    if !member.can_withdraw(&pool.prices(), amount, is_mega, depositor.owner)? {
        return Err(RegistryErrorCode::InsufficientBalance)?;
    }

    Ok(AccessControlResponse { depositor })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: withdraw");

    let StateTransitionRequest {
        entity,
        member,
        amount,
        registrar,
        registrar_acc_info,
        vault_authority_acc_info,
        depositor_acc_info,
        depositor,
        vault_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer funds from the program vault back to the original depositor.
    invoke_token_transfer(
        vault_acc_info,
        depositor_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
        amount,
    )?;

    let is_mega = registrar.is_mega(*vault_acc_info.key)?;
    member.did_withdraw(amount, is_mega, depositor.owner);
    entity.did_withdraw(amount, is_mega);

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    depositor_authority_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    pool: &'c Pool<'a, 'b>,
    registrar: &'c Registrar,
    amount: u64,
}

struct AccessControlResponse {
    depositor: TokenAccount,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    entity: &'c mut Entity,
    member: &'c mut Member,
    registrar: &'c Registrar,
    depositor: &'c TokenAccount,
    amount: u64,
}

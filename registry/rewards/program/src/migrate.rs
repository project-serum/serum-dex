use crate::access_control;
use serum_common::pack::*;
use serum_common::program::invoke_token_transfer;
use serum_registry_rewards::accounts::{vault, Instance};
use serum_registry_rewards::error::RewardsError;
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RewardsError> {
    msg!("handler: migrate");

    let acc_infos = &mut accounts.iter();

    let instance_authority_acc_info = next_account_info(acc_infos)?;
    let instance_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let receiver_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { instance, vault } = access_control(AccessControlRequest {
        program_id,
        instance_acc_info,
        vault_acc_info,
        instance_authority_acc_info,
        vault_authority_acc_info,
    })?;

    state_transition(StateTransitionRequest {
        instance,
        instance_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        vault,
        receiver_acc_info,
        token_program_acc_info,
    })
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RewardsError> {
    msg!("access-control: migrate");

    let AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
    } = req;

    let instance = access_control::instance(instance_acc_info, program_id)?;
    let vault = access_control::vault(
        vault_acc_info,
        vault_authority_acc_info,
        &instance,
        instance_acc_info,
        program_id,
    )?;

    // Authorization.
    access_control::governance(program_id, instance_authority_acc_info, &instance)?;

    Ok(AccessControlResponse { instance, vault })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    msg!("state-transition: migrate");

    let StateTransitionRequest {
        instance,
        instance_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        vault,
        receiver_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer all tokens to the new account.
    invoke_token_transfer(
        vault_acc_info,
        receiver_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(instance_acc_info.key, &instance.nonce)],
        vault.amount,
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    instance_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    instance_authority_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
}

struct AccessControlResponse {
    instance: Instance,
    vault: TokenAccount,
}

struct StateTransitionRequest<'a, 'b> {
    instance: Instance,
    instance_acc_info: &'a AccountInfo<'b>,
    vault: TokenAccount,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    receiver_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
}

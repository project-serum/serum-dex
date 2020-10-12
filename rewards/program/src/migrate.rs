use crate::access_control;
use serum_common::pack::*;
use serum_rewards::accounts::{vault, Instance};
use serum_rewards::error::RewardsError;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
) -> Result<(), RewardsError> {
    info!("handler: migrate");

    let acc_infos = &mut accounts.iter();

    let instance_authority_acc_info = next_account_info(acc_infos)?;
    let instance_acc_info = next_account_info(acc_infos)?;
    let instance_vault_acc_info = next_account_info(acc_infos)?;
    let instance_vault_authority_acc_info = next_account_info(acc_infos)?;
    let receiver_spl_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    })?;

    let instance = Instance::unpack(&instance_acc_info.try_borrow_data()?)?;
    let instance_vault =
        spl_token::state::Account::unpack(&instance_vault_acc_info.try_borrow_data()?)?;
    state_transition(StateTransitionRequest {
        instance,
        instance_acc_info,
        instance_vault_acc_info,
        instance_vault_authority_acc_info,
        instance_vault_amount: instance_vault.amount,
        receiver_spl_acc_info,
        token_program_acc_info,
    })
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    info!("access-control: migrate");

    let AccessControlRequest {
        program_id,
        instance_acc_info,
        instance_authority_acc_info,
    } = req;

    let _ = access_control::governance(program_id, instance_acc_info, instance_authority_acc_info)?;

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: migrate");

    let StateTransitionRequest {
        instance,
        instance_acc_info,
        instance_vault_acc_info,
        instance_vault_authority_acc_info,
        instance_vault_amount,
        receiver_spl_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer all tokens to the new account.
    {
        info!("invoking migration token transfer");

        let withdraw_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            instance_vault_acc_info.key,
            receiver_spl_acc_info.key,
            &instance_vault_authority_acc_info.key,
            &[],
            instance_vault_amount,
        )?;

        let seeds = vault::signer_seeds(instance_acc_info.key, &instance.nonce);
        let accs = vec![
            instance_vault_acc_info.clone(),
            receiver_spl_acc_info.clone(),
            instance_vault_authority_acc_info.clone(),
            token_program_acc_info.clone(),
        ];
        solana_sdk::program::invoke_signed(&withdraw_instruction, &accs, &[&seeds])?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    instance_acc_info: &'a AccountInfo<'a>,
    instance_authority_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    instance: Instance,
    instance_acc_info: &'a AccountInfo<'a>,
    instance_vault_amount: u64,
    instance_vault_acc_info: &'a AccountInfo<'a>,
    instance_vault_authority_acc_info: &'a AccountInfo<'a>,
    receiver_spl_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

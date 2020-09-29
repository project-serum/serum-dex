use serum_registry::accounts::{registry, vault};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use serum_registry::rewards;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
) -> Result<(), RegistryError> {
    info!("handler: collect_rewards");

    let acc_infos = &mut accounts.iter();

    let rewards_program_acc_info = next_account_info(acc_infos)?;
    let rewards_rv_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let stake_beneficiary_acc_info = next_account_info(acc_infos)?;
    let beneficiary_tok_acc_info = next_account_info(acc_infos)?;
    let stake_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registry_acc_info = next_account_info(acc_infos)?;
    let tok_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        rewards_program_acc_info,
        rewards_rv_acc_info,
        stake_beneficiary_acc_info,
        beneficiary_tok_acc_info,
        stake_acc_info,
        entity_acc_info,
        registry_acc_info,
        tok_program_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
    })?;

    state_transition(StateTransitionRequest {
        accounts,
        rewards_program_acc_info,
        rewards_rv_acc_info,
        beneficiary_tok_acc_info,
        stake_acc_info,
        entity_acc_info,
        registry_acc_info,
        tok_program_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
    })
    .map_err(Into::into)
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: collect_rewards");

    let AccessControlRequest {
        rewards_program_acc_info,
        rewards_rv_acc_info,
        stake_beneficiary_acc_info,
        beneficiary_tok_acc_info,
        stake_acc_info,
        entity_acc_info,
        registry_acc_info,
        tok_program_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: collect_rewards");

    let StateTransitionRequest {
        accounts,
        rewards_program_acc_info,
        rewards_rv_acc_info,
        beneficiary_tok_acc_info,
        stake_acc_info,
        entity_acc_info,
        registry_acc_info,
        tok_program_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
    } = req;

    // Calculate the reward.
    let reward_amount = {
        info!("invoke rewards calculation");

        let rewards_instr = rewards::instruction(
            rewards_program_acc_info.key,
            &[
                AccountMeta::new(*rewards_rv_acc_info.key, false),
                AccountMeta::new_readonly(*registry_acc_info.key, false),
                AccountMeta::new_readonly(*stake_acc_info.key, false),
                AccountMeta::new_readonly(*entity_acc_info.key, false),
            ],
        );

        solana_sdk::program::invoke(&rewards_instr, &accounts[..])?;

        let mut dst = [0u8; 8];
        dst.copy_from_slice(&rewards_rv_acc_info.try_borrow_data()?);
        u64::from_le_bytes(dst)
    };

    // Send the funds.
    {
        info!("invoke SPL token transfer");
        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            beneficiary_tok_acc_info.key,
            vault_authority_acc_info.key,
            &[],
            reward_amount,
        )?;

        let data = registry_acc_info.try_borrow_data()?;
        let nonce = data[registry::NONCE_INDEX];
        let signer_seeds = vault::signer_seeds(registry_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                vault_authority_acc_info.clone(),
                beneficiary_tok_acc_info.clone(),
                tok_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    rewards_program_acc_info: &'a AccountInfo<'a>,
    rewards_rv_acc_info: &'a AccountInfo<'a>,
    stake_beneficiary_acc_info: &'a AccountInfo<'a>,
    beneficiary_tok_acc_info: &'a AccountInfo<'a>,
    stake_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    tok_program_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    vault_authority_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    accounts: &'a [AccountInfo<'a>],
    rewards_program_acc_info: &'a AccountInfo<'a>,
    rewards_rv_acc_info: &'a AccountInfo<'a>,
    stake_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    beneficiary_tok_acc_info: &'a AccountInfo<'a>,
    tok_program_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    vault_authority_acc_info: &'a AccountInfo<'a>,
}

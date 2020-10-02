use serum_common::pack::Pack;
use serum_registry::accounts::{Entity, Stake};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    beneficiary: Pubkey,
    is_mega: bool,
) -> Result<(), RegistryError> {
    info!("handler: stake");

    let acc_infos = &mut accounts.iter();

    let stake_acc_info = next_account_info(acc_infos)?;
    let staker_tok_authority_acc_info = next_account_info(acc_infos)?;
    let staker_tok_acc_info = next_account_info(acc_infos)?;
    let registry_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        staker_tok_authority_acc_info,
        staker_tok_acc_info,
        stake_acc_info,
        registry_acc_info,
        vault_acc_info,
        entity_acc_info,
        token_program_acc_info,
        is_mega,
    })?;

    Entity::unpack_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            Stake::unpack_mut(
                &mut stake_acc_info.try_borrow_mut_data()?,
                &mut |stake: &mut Stake| {
                    state_transition(StateTransitionRequest {
                        entity,
                        stake,
                        entity_acc_info,
                        staker_tok_authority_acc_info,
                        staker_tok_acc_info,
                        vault_acc_info,
                        token_program_acc_info,
                        amount,
                        beneficiary,
                        is_mega,
                    })
                    .map_err(Into::into)
                },
            )
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: stake");

    let AccessControlRequest {
        staker_tok_authority_acc_info,
        staker_tok_acc_info,
        stake_acc_info,
        registry_acc_info,
        vault_acc_info,
        entity_acc_info,
        token_program_acc_info,
        is_mega,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        stake,
        entity_acc_info,
        staker_tok_authority_acc_info,
        staker_tok_acc_info,
        vault_acc_info,
        token_program_acc_info,
        amount,
        beneficiary,
        is_mega,
    } = req;

    // Stake account.
    {
        stake.initialized = true;
        stake.beneficiary = beneficiary;
        stake.entity_id = *entity_acc_info.key;
        stake.amount = 0;
        stake.mega_amount = 0;

        if is_mega {
            stake.mega_amount = amount;
        } else {
            stake.amount = amount;
        }
    }

    // Entity.
    {
        if is_mega {
            entity.mega_amount += amount;
        } else {
            entity.amount += amount;
        }
    }

    // Transfer funds from staker to vault.
    {
        let deposit_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            staker_tok_acc_info.key,
            vault_acc_info.key,
            staker_tok_authority_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(
            &deposit_instruction,
            &[
                staker_tok_acc_info.clone(),
                staker_tok_authority_acc_info.clone(),
                vault_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[],
        )?;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    staker_tok_authority_acc_info: &'a AccountInfo<'a>,
    staker_tok_acc_info: &'a AccountInfo<'a>,
    stake_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    is_mega: bool,
}

struct StateTransitionRequest<'a, 'b> {
    entity: &'b mut Entity,
    stake: &'b mut Stake,
    entity_acc_info: &'a AccountInfo<'a>,
    staker_tok_authority_acc_info: &'a AccountInfo<'a>,
    staker_tok_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    amount: u64,
    beneficiary: Pubkey,
    is_mega: bool,
}

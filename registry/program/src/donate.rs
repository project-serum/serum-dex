use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

// TODO: update to transfer funds to the pool.
pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    donate_amount: u64,
) -> Result<(), RegistryError> {
    info!("handler: donate");

    let acc_infos = &mut accounts.iter();

    let donator_authority_acc_info = next_account_info(acc_infos)?;
    let donator_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let registry_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        donator_authority_acc_info,
        donator_acc_info,
        vault_acc_info,
        registry_acc_info,
        token_program_acc_info,
    })?;

    state_transition(StateTransitionRequest {
        donator_authority_acc_info,
        donator_acc_info,
        vault_acc_info,
        registry_acc_info,
        token_program_acc_info,
        donate_amount,
    })
    .map_err(Into::into)
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), RegistryError> {
    info!("access-control: donate");

    let AccessControlRequest {
        donator_authority_acc_info,
        donator_acc_info,
        vault_acc_info,
        registry_acc_info,
        token_program_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a>(req: StateTransitionRequest<'a>) -> Result<(), RegistryError> {
    info!("state-transition: donate");

    let StateTransitionRequest {
        donator_authority_acc_info,
        donator_acc_info,
        vault_acc_info,
        registry_acc_info,
        token_program_acc_info,
        donate_amount,
    } = req;

    info!("invoke SPL token transfer");

    let donate_instruction = spl_token::instruction::transfer(
        &spl_token::ID,
        donator_acc_info.key,
        vault_acc_info.key,
        donator_authority_acc_info.key,
        &[],
        donate_amount,
    )?;
    solana_sdk::program::invoke_signed(
        &donate_instruction,
        &[
            donator_acc_info.clone(),
            donator_authority_acc_info.clone(),
            vault_acc_info.clone(),
            token_program_acc_info.clone(),
        ],
        &[],
    )?;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    donator_authority_acc_info: &'a AccountInfo<'a>,
    donator_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a> {
    donator_authority_acc_info: &'a AccountInfo<'a>,
    donator_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    registry_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    donate_amount: u64,
}

use serum_common::program::invoke_token_transfer;
use serum_registry::accounts::reward_queue::Ring;
use serum_registry::accounts::{RewardEvent, RewardEventQueue};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::next_account_info;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    totals: Vec<u64>,
) -> Result<(), RegistryError> {
    info!("handler: drop_pool_reward");

    let acc_infos = &mut accounts.iter();

    let reward_event_q_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let mut depositor_acc_infos = vec![next_account_info(acc_infos)?];
    if accounts.len() == 9 {
        depositor_acc_infos.push(next_account_info(acc_infos)?);
    }
    let depositor_owner_acc_info = next_account_info(acc_infos)?;
    let pool_acc_info = next_account_info(acc_infos)?;
    let mut pool_vault_acc_infos = vec![next_account_info(acc_infos)?];
    if accounts.len() == 9 {
        pool_vault_acc_infos.push(next_account_info(acc_infos)?);
    }
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        reward_event_q_acc_info,
        pool_acc_info,
        pool_vault_acc_infos: &pool_vault_acc_infos,
    })?;

    state_transition(StateTransitionRequest {
        totals,
        pool_acc_info,
        pool_vault_acc_infos: &pool_vault_acc_infos,
        depositor_acc_infos: &depositor_acc_infos,
        depositor_owner_acc_info,
        token_program_acc_info,
        reward_event_q_acc_info,
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: drop_pool_reward");

    let AccessControlRequest {
        registrar_acc_info,
        reward_event_q_acc_info,
        pool_acc_info: _,
        pool_vault_acc_infos: _,
    } = req;

    // todo

    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    if event_q.authority() != *registrar_acc_info.key {
        return Err(RegistryErrorCode::InvalidRewardQueueAuthority)?;
    }

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: drop_pool_reward");

    let StateTransitionRequest {
        totals,
        pool_acc_info,
        pool_vault_acc_infos,
        depositor_acc_infos,
        depositor_owner_acc_info,
        token_program_acc_info,
        reward_event_q_acc_info,
    } = req;

    invoke_token_transfer(
        depositor_acc_infos[0],
        pool_vault_acc_infos[0],
        depositor_owner_acc_info,
        token_program_acc_info,
        &[],
        totals[0],
    )?;
    if totals.len() == 2 {
        invoke_token_transfer(
            depositor_acc_infos[1],
            pool_vault_acc_infos[1],
            depositor_owner_acc_info,
            token_program_acc_info,
            &[],
            totals[1],
        )?;
    }

    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    let event = RewardEvent::PoolDrop {
        from: *depositor_owner_acc_info.key,
        totals,
        pool: *pool_acc_info.key,
    };
    event_q.append(&event)?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    registrar_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    pool_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_infos: &'c [&'a AccountInfo<'b>],
}

struct StateTransitionRequest<'a, 'b, 'c> {
    totals: Vec<u64>,
    pool_acc_info: &'a AccountInfo<'b>,
    pool_vault_acc_infos: &'c [&'a AccountInfo<'b>],
    depositor_acc_infos: &'c [&'a AccountInfo<'b>],
    depositor_owner_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
}

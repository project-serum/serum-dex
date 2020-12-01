use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::reward_queue::{RewardEventQueue, Ring};
use serum_registry::accounts::Registrar;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: Pubkey,
    nonce: u8,
    withdrawal_timelock: i64,
    deactivation_timelock: i64,
    reward_activation_threshold: u64,
    max_stake_per_entity: u64,
) -> Result<(), RegistryError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let registrar_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let mega_vault_acc_info = next_account_info(acc_infos)?;
    let pool_acc_info = next_account_info(acc_infos)?;
    let mega_pool_acc_info = next_account_info(acc_infos)?;
    let pool_program_acc_info = next_account_info(acc_infos)?;
    let reward_event_q_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        vault_acc_info,
        mega_vault_acc_info,
        program_id,
        nonce,
        reward_event_q_acc_info,
    })?;

    Registrar::unpack_mut(
        &mut registrar_acc_info.try_borrow_mut_data()?,
        &mut |registrar: &mut Registrar| {
            state_transition(StateTransitionRequest {
                registrar,
                authority,
                vault_acc_info,
                mega_vault_acc_info,
                withdrawal_timelock,
                nonce,
                deactivation_timelock,
                reward_activation_threshold,
                max_stake_per_entity,
                pool_program_acc_info, // Not validated.
                pool_acc_info,         // Not validated.
                mega_pool_acc_info,    // Not validated.
                reward_event_q_acc_info,
                registrar_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        program_id,
        vault_acc_info,
        mega_vault_acc_info,
        nonce,
        reward_event_q_acc_info,
    } = req;

    // Authorization: none.

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;

    // Registrar (uninitialized).
    {
        let registrar = Registrar::unpack(&registrar_acc_info.try_borrow_data()?)?;
        if registrar_acc_info.owner != program_id {
            return Err(RegistryErrorCode::InvalidOwner)?;
        }
        if !rent.is_exempt(
            registrar_acc_info.lamports(),
            registrar_acc_info.try_data_len()?,
        ) {
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
        if registrar.initialized {
            return Err(RegistryErrorCode::AlreadyInitialized)?;
        }
    }

    // Vaults (initialized but not yet on the Registrar).
    access_control::vault_init(vault_acc_info, registrar_acc_info, &rent, nonce, program_id)?;
    access_control::vault_init(
        mega_vault_acc_info,
        registrar_acc_info,
        &rent,
        nonce,
        program_id,
    )?;

    // Reward q must not yet be owned.
    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    if event_q.authority() != Pubkey::new_from_array([0u8; 32]) {
        return Err(RegistryErrorCode::RewardQAlreadyOwned)?;
    }

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        registrar,
        authority,
        withdrawal_timelock,
        vault_acc_info,
        mega_vault_acc_info,
        nonce,
        deactivation_timelock,
        reward_activation_threshold,
        pool_acc_info,
        pool_program_acc_info,
        mega_pool_acc_info,
        max_stake_per_entity,
        reward_event_q_acc_info,
        registrar_acc_info,
    } = req;

    registrar.initialized = true;
    registrar.authority = authority;
    registrar.withdrawal_timelock = withdrawal_timelock;
    registrar.deactivation_timelock = deactivation_timelock;
    registrar.max_stake_per_entity = max_stake_per_entity;
    registrar.vault = *vault_acc_info.key;
    registrar.mega_vault = *mega_vault_acc_info.key;
    registrar.nonce = nonce;
    registrar.reward_activation_threshold = reward_activation_threshold;
    registrar.pool = *pool_acc_info.key;
    registrar.mega_pool = *mega_pool_acc_info.key;
    registrar.pool_program_id = *pool_program_acc_info.key;
    registrar.reward_event_q = *reward_event_q_acc_info.key;

    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    event_q.set_authority(registrar_acc_info.key);

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    registrar_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    nonce: u8,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    vault_acc_info: &'a AccountInfo<'b>,
    mega_vault_acc_info: &'a AccountInfo<'b>,
    mega_pool_acc_info: &'a AccountInfo<'b>,
    pool_acc_info: &'a AccountInfo<'b>,
    pool_program_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c mut Registrar,
    authority: Pubkey,
    reward_activation_threshold: u64,
    deactivation_timelock: i64,
    withdrawal_timelock: i64,
    max_stake_per_entity: u64,
    nonce: u8,
}

use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::reward_queue::{RewardEventQueue, Ring};
use serum_registry::accounts::{vault, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::program_option::COption;
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    mint: Pubkey,
    mint_mega: Pubkey,
    authority: Pubkey,
    nonce: u8,
    withdrawal_timelock: i64,
    deactivation_timelock: i64,
    max_stake_per_entity: u64,
    stake_rate: u64,
    stake_rate_mega: u64,
) -> Result<(), RegistryError> {
    msg!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let registrar_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let pool_mint_mega_acc_info = next_account_info(acc_infos)?;
    let reward_event_q_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        pool_mint_acc_info,
        pool_mint_mega_acc_info,
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
                mint,
                mint_mega,
                pool_mint_acc_info,
                pool_mint_mega_acc_info,
                withdrawal_timelock,
                nonce,
                deactivation_timelock,
                max_stake_per_entity,
                reward_event_q_acc_info,
                registrar_acc_info,
                stake_rate,
                stake_rate_mega,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    msg!("access-control: initialize");

    let AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        pool_mint_acc_info,
        pool_mint_mega_acc_info,
        program_id,
        nonce,
        reward_event_q_acc_info,
    } = req;

    // Authorization: none.

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let pool_mint = access_control::mint(pool_mint_acc_info)?;
    let pool_mint_mega = access_control::mint(pool_mint_mega_acc_info)?;
    let vault_authority = Pubkey::create_program_address(
        &vault::signer_seeds(registrar_acc_info.key, &nonce),
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultNonce)?;
    if pool_mint.mint_authority != COption::Some(vault_authority)
        || pool_mint_mega.mint_authority != COption::Some(vault_authority)
    {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }

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

    // Reward q must not yet be owned.
    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    if event_q.get_init()? {
        return Err(RegistryErrorCode::RewardQAlreadyOwned)?;
    }

    Ok(())
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: initialize");

    let StateTransitionRequest {
        registrar,
        authority,
        withdrawal_timelock,
        nonce,
        deactivation_timelock,
        pool_mint_acc_info,
        pool_mint_mega_acc_info,
        max_stake_per_entity,
        reward_event_q_acc_info,
        registrar_acc_info,
        mint,
        mint_mega,
        stake_rate,
        stake_rate_mega,
    } = req;

    registrar.initialized = true;
    registrar.authority = authority;
    registrar.nonce = nonce;
    registrar.mint = mint;
    registrar.mega_mint = mint_mega;
    registrar.pool_mint = *pool_mint_acc_info.key;
    registrar.pool_mint_mega = *pool_mint_mega_acc_info.key;
    registrar.stake_rate = stake_rate;
    registrar.stake_rate_mega = stake_rate_mega;
    registrar.reward_event_q = *reward_event_q_acc_info.key;
    registrar.withdrawal_timelock = withdrawal_timelock;
    registrar.deactivation_timelock = deactivation_timelock;
    registrar.max_stake_per_entity = max_stake_per_entity;

    let event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    event_q.set_init()?;
    event_q.set_authority(registrar_acc_info.key);

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    registrar_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    pool_mint_mega_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    nonce: u8,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    pool_mint_mega_acc_info: &'a AccountInfo<'b>,
    registrar: &'c mut Registrar,
    authority: Pubkey,
    deactivation_timelock: i64,
    withdrawal_timelock: i64,
    max_stake_per_entity: u64,
    nonce: u8,
    mint: Pubkey,
    mint_mega: Pubkey,
    stake_rate: u64,
    stake_rate_mega: u64,
}

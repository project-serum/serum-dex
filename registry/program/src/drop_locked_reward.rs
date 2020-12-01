use crate::common::invoke_token_transfer;
use borsh::BorshDeserialize;
use serum_common::pack::Pack;
use serum_pool_schema::PoolState;
use serum_registry::access_control;
use serum_registry::accounts::reward_queue::Ring;
use serum_registry::accounts::{LockedRewardVendor, RewardEvent, RewardEventQueue};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::next_account_info;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use spl_token::state::{Account as TokenAccount, Mint};
use std::convert::Into;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    total: u64,
    end_ts: i64,
    expiry_ts: i64,
    expiry_receiver: Pubkey,
    period_count: u64,
    nonce: u8,
) -> Result<(), RegistryError> {
    info!("handler: drop_locked_reward");

    let acc_infos = &mut accounts.iter();

    let reward_event_q_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_owner_acc_info = next_account_info(acc_infos)?;
    let pool_acc_info = next_account_info(acc_infos)?;
    let pool_token_mint_acc_info = next_account_info(acc_infos)?;
    let locked_vendor_acc_info = next_account_info(acc_infos)?;
    let locked_vendor_vault_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse {
        pool_token_mint,
        ref clock,
    } = access_control(AccessControlRequest {
        program_id,
        registrar_acc_info,
        pool_acc_info,
        pool_token_mint_acc_info,
        locked_vendor_acc_info,
        clock_acc_info,
        expiry_ts,
        end_ts,
    })?;

    LockedRewardVendor::unpack_mut(
        &mut locked_vendor_acc_info.try_borrow_mut_data()?,
        &mut |locked_vendor: &mut LockedRewardVendor| {
            state_transition(StateTransitionRequest {
                nonce,
                total,
                end_ts,
                expiry_ts,
                expiry_receiver,
                locked_vendor,
                locked_vendor_acc_info,
                locked_vendor_vault_acc_info,
                reward_event_q_acc_info,
                registrar_acc_info,
                pool_acc_info,
                depositor_acc_info,
                depositor_owner_acc_info,
                token_program_acc_info,
                pool_token_mint,
                clock,
                period_count,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: drop_locked_reward");

    let AccessControlRequest {
        program_id,
        registrar_acc_info,
        pool_acc_info,
        pool_token_mint_acc_info,
        locked_vendor_acc_info,
        clock_acc_info,
        expiry_ts,
        end_ts,
    } = req;

    // Authorization: none.

    // Account validation.
    let clock = access_control::clock(clock_acc_info)?;
    //
    // Registrar.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    if &registrar.pool != pool_acc_info.key && &registrar.mega_pool != pool_acc_info.key {
        return Err(RegistryErrorCode::InvalidPoolAccounts)?;
    }
    // Pool.
    let data = pool_acc_info.try_borrow_data()?;
    let mut data: &[u8] = *data;
    let pool =
        PoolState::deserialize(&mut data).map_err(|_| RegistryErrorCode::WrongSerialization)?;
    let pt_address: Pubkey = pool.pool_token_mint.into();
    let pool_token_mint = access_control::mint(pool_token_mint_acc_info)?;
    if pt_address != *pool_token_mint_acc_info.key {
        return Err(RegistryErrorCode::InvalidPoolTokenMint)?;
    }

    // DropLockedReward specific.
    if LockedRewardVendor::initialized(locked_vendor_acc_info)? {
        return Err(RegistryErrorCode::AlreadyInitialized)?;
    }
    if clock.unix_timestamp >= end_ts {
        return Err(RegistryErrorCode::InvalidEndTs)?;
    }
    if clock.unix_timestamp >= expiry_ts {
        return Err(RegistryErrorCode::InvalidExpiry)?;
    }

    // TODO: enforce a *minimum* reward drop to prevent flooding the queue.

    Ok(AccessControlResponse {
        pool_token_mint,
        clock,
    })
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: drop_locked_reward");

    let StateTransitionRequest {
        nonce,
        total,
        expiry_ts,
        expiry_receiver,
        locked_vendor,
        locked_vendor_acc_info,
        locked_vendor_vault_acc_info,
        reward_event_q_acc_info,
        registrar_acc_info,
        pool_acc_info,
        depositor_acc_info,
        depositor_owner_acc_info,
        token_program_acc_info,
        pool_token_mint,
        clock,
        period_count,
        end_ts,
    } = req;

    let mint = {
        let t = TokenAccount::unpack(&depositor_acc_info.try_borrow_data()?)?;
        t.mint
    };

    // Emit a reward event.
    let reward_event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    let cursor = reward_event_q.head_cursor()?;
    reward_event_q.append(&RewardEvent::LockedAlloc {
        from: *depositor_owner_acc_info.key,
        pool: *pool_acc_info.key,
        total,
        locked_vendor: *locked_vendor_acc_info.key,
        mint,
    })?;

    // Transfer the reward to the vendor.
    invoke_token_transfer(
        depositor_acc_info,
        locked_vendor_vault_acc_info,
        depositor_owner_acc_info,
        token_program_acc_info,
        &[],
        total,
    )?;

    // Initialize the reward vendor account.
    locked_vendor.initialized = true;
    locked_vendor.registrar = *registrar_acc_info.key;
    locked_vendor.vault = *locked_vendor_vault_acc_info.key;
    locked_vendor.nonce = nonce;
    locked_vendor.pool_token_supply = pool_token_mint.supply;
    locked_vendor.reward_event_q_cursor = cursor;
    locked_vendor.start_ts = clock.unix_timestamp;
    locked_vendor.end_ts = end_ts;
    locked_vendor.expiry_ts = expiry_ts;
    locked_vendor.expiry_receiver = expiry_receiver;
    locked_vendor.total = total;
    locked_vendor.period_count = period_count;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    registrar_acc_info: &'a AccountInfo<'b>,
    pool_acc_info: &'a AccountInfo<'b>,
    pool_token_mint_acc_info: &'a AccountInfo<'b>,
    locked_vendor_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    expiry_ts: i64,
    end_ts: i64,
}

struct AccessControlResponse {
    pool_token_mint: Mint,
    clock: Clock,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    nonce: u8,
    total: u64,
    end_ts: i64,
    expiry_ts: i64,
    period_count: u64,
    expiry_receiver: Pubkey,
    clock: &'c Clock,
    locked_vendor: &'c mut LockedRewardVendor,
    locked_vendor_acc_info: &'a AccountInfo<'b>,
    locked_vendor_vault_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    pool_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    depositor_owner_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    pool_token_mint: Mint,
}
